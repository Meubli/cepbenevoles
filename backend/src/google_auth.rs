//! Service-account OAuth2 for the Google Sheets API (technical-specs.md §1).
//!
//! No external OAuth crate: a service-account JWT-bearer exchange is small
//! enough to sign ourselves and keeps `token_uri` overridable for tests
//! (point it at a wiremock server instead of Google).

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceAccountKey {
    pub client_email: String,
    pub private_key: String,
    pub token_uri: String,
}

impl ServiceAccountKey {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[derive(Debug, Serialize)]
struct Claims {
    iss: String,
    scope: String,
    aud: String,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid service account private key: {0}")]
    InvalidKey(#[from] jsonwebtoken::errors::Error),
    #[error("token request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("token endpoint returned {status}: {body}")]
    TokenRejected { status: u16, body: String },
}

struct CachedToken {
    access_token: String,
    /// Unix seconds; refresh once "now" gets within 60s of this.
    expires_at: u64,
}

/// Fetches and caches Sheets-readonly access tokens for one service account,
/// refreshing ~60s before expiry. Shared behind an `Arc` in `AppState`.
pub struct GoogleAuth {
    key: ServiceAccountKey,
    http: reqwest::Client,
    cached: Mutex<Option<CachedToken>>,
}

const SCOPE: &str = "https://www.googleapis.com/auth/spreadsheets.readonly";
const REFRESH_SKEW_SECS: u64 = 60;

impl GoogleAuth {
    pub fn new(key: ServiceAccountKey) -> Arc<Self> {
        Arc::new(Self {
            key,
            http: reqwest::Client::new(),
            cached: Mutex::new(None),
        })
    }

    pub async fn access_token(&self) -> Result<String, AuthError> {
        let now = unix_now();

        {
            let cached = self.cached.lock().await;
            if let Some(t) = cached.as_ref() {
                if t.expires_at > now + REFRESH_SKEW_SECS {
                    return Ok(t.access_token.clone());
                }
            }
        }

        let fresh = self.request_token(now).await?;
        let access_token = fresh.0.clone();
        *self.cached.lock().await = Some(CachedToken {
            access_token: fresh.0,
            expires_at: now + fresh.1,
        });
        Ok(access_token)
    }

    async fn request_token(&self, now: u64) -> Result<(String, u64), AuthError> {
        let claims = Claims {
            iss: self.key.client_email.clone(),
            scope: SCOPE.to_string(),
            aud: self.key.token_uri.clone(),
            iat: now,
            exp: now + 3600,
        };
        let encoding_key = EncodingKey::from_rsa_pem(self.key.private_key.as_bytes())?;
        let jwt = jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &encoding_key)?;

        let resp = self
            .http
            .post(&self.key.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", jwt.as_str()),
            ])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AuthError::TokenRejected {
                status: status.as_u16(),
                body,
            });
        }

        let token: TokenResponse = resp.json().await?;
        Ok((token.access_token, token.expires_in))
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Freshly generated for this test only — never used for anything real.
    const TEST_PRIVATE_KEY: &str = include_str!("../tests/fixtures/test_rsa_private_key.pem");

    fn test_key(token_uri: String) -> ServiceAccountKey {
        ServiceAccountKey {
            client_email: "test@example.iam.gserviceaccount.com".to_string(),
            private_key: TEST_PRIVATE_KEY.to_string(),
            token_uri,
        }
    }

    #[tokio::test]
    async fn fetches_and_caches_the_token() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "fake-access-token",
                "expires_in": 3600,
            })))
            .expect(1) // cached on the second call, not refetched
            .mount(&mock_server)
            .await;

        let auth = GoogleAuth::new(test_key(format!("{}/token", mock_server.uri())));

        let first = auth.access_token().await.unwrap();
        let second = auth.access_token().await.unwrap();

        assert_eq!(first, "fake-access-token");
        assert_eq!(second, "fake-access-token");
    }

    #[tokio::test]
    async fn token_rejection_is_a_typed_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
            .mount(&mock_server)
            .await;

        let auth = GoogleAuth::new(test_key(format!("{}/token", mock_server.uri())));

        let err = auth.access_token().await.unwrap_err();
        assert!(matches!(err, AuthError::TokenRejected { status: 400, .. }));
    }
}
