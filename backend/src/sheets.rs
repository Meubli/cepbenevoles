//! Google Sheets API client.

use reqwest::StatusCode;
use serde::Deserialize;

#[derive(Clone)]
pub struct SheetsClient {
    http: reqwest::Client,
    /// Overridable so tests point this at a wiremock server instead of
    /// `https://sheets.googleapis.com`.
    base_url: String,
}

#[derive(Deserialize)]
struct ValuesResponse {
    #[serde(default)]
    values: Vec<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum SheetsError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// The Sheet doesn't exist (or the id is wrong) — distinct from
    /// `Forbidden` so the Événements screen can tell the admin which one to
    /// fix (specs.md §3.3).
    #[error("spreadsheet not found")]
    NotFound,
    /// The Sheet exists but isn't shared with the service account.
    #[error("access to spreadsheet forbidden — not shared with the service account")]
    Forbidden,
    #[error("unexpected status {0} from Sheets API: {1}")]
    UnexpectedStatus(StatusCode, String),
}

impl SheetsClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Fetches every value in one tab (e.g. `"Réponses au formulaire 1"`),
    /// using an open `<tab>!A1:ZZ` range rather than a fixed last column, so
    /// a question added later isn't silently cut off. Returns rows of cell
    /// strings as the Sheets API reports them (trailing empty cells in a
    /// row are omitted by the API itself, not by this client).
    pub async fn fetch_values(
        &self,
        access_token: &str,
        spreadsheet_id: &str,
        tab_name: &str,
    ) -> Result<Vec<Vec<String>>, SheetsError> {
        let range = format!("{tab_name}!A1:ZZ");
        let mut url = reqwest::Url::parse(&self.base_url)
            .expect("configured base_url must be a valid URL")
            .join(&format!("v4/spreadsheets/{spreadsheet_id}/values"))
            .expect("spreadsheet_id must be URL-safe");
        url.path_segments_mut()
            .expect("base_url must be able to be a base")
            .push(&range);

        let resp = self.http.get(url).bearer_auth(access_token).send().await?;

        match resp.status() {
            StatusCode::OK => Ok(resp.json::<ValuesResponse>().await?.values),
            StatusCode::NOT_FOUND => Err(SheetsError::NotFound),
            StatusCode::FORBIDDEN => Err(SheetsError::Forbidden),
            other => {
                let body = resp.text().await.unwrap_or_default();
                Err(SheetsError::UnexpectedStatus(other, body))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetches_values_with_a_properly_encoded_range() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "values": [["Jean", "Dupont"]]
            })))
            .mount(&mock_server)
            .await;

        let client = SheetsClient::new(mock_server.uri());
        let rows = client
            .fetch_values("fake-token", "fake-sheet-id", "Réponses au formulaire 1")
            .await
            .expect("fetch should succeed");

        assert_eq!(rows, vec![vec!["Jean".to_string(), "Dupont".to_string()]]);

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        // The real tab name has spaces and an accent — assert the request
        // actually reached the server with the range properly encoded,
        // rather than trusting fetch_values() built a valid URL at all.
        assert_eq!(
            requests[0].url.path(),
            "/v4/spreadsheets/fake-sheet-id/values/R%C3%A9ponses%20au%20formulaire%201!A1:ZZ"
        );
    }

    #[tokio::test]
    async fn not_found_is_distinct_from_forbidden() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = SheetsClient::new(mock_server.uri());
        let err = client
            .fetch_values("fake-token", "missing-sheet", "Tab")
            .await
            .unwrap_err();

        assert!(matches!(err, SheetsError::NotFound));
    }

    #[tokio::test]
    async fn forbidden_means_not_shared() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        let client = SheetsClient::new(mock_server.uri());
        let err = client
            .fetch_values("fake-token", "private-sheet", "Tab")
            .await
            .unwrap_err();

        assert!(matches!(err, SheetsError::Forbidden));
    }
}
