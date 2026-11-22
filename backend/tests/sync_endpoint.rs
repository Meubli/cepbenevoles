mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use benefrance_backend::google_auth::{GoogleAuth, ServiceAccountKey};
use benefrance_backend::sheets::SheetsClient;
use benefrance_backend::{app, AppState, SyncContext};
use common::spawn_test_db;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::Mutex;
use tower::ServiceExt;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_PRIVATE_KEY: &str = include_str!("fixtures/test_rsa_private_key.pem");

fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

async fn insert_session(pool: &PgPool) -> String {
    let admin_id: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO admins (email) VALUES ('admin@example.org') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    let token = "test-session-token";
    sqlx::query(
        "INSERT INTO sessions (admin_id, token_hash, expires_at) \
         VALUES ($1, $2, now() + interval '1 day')",
    )
    .bind(admin_id)
    .bind(hash(token))
    .execute(pool)
    .await
    .unwrap();

    token.to_string()
}

fn base_state(pool: PgPool, sync: Option<SyncContext>) -> AppState {
    AppState {
        db: pool,
        smtp_host: "localhost".to_string(),
        smtp_port: 2525,
        smtp_user: None,
        smtp_pass: None,
        smtp_from: "noreply@example.org".to_string(),
        sync,
        session_cookie_secure: true,
        sheet_validation_required: true,
    }
}

#[tokio::test]
async fn trigger_sync_requires_a_session() {
    let pool = spawn_test_db().await;
    let state = base_state(pool, None);
    let router = app(state, None);

    let response = router
        .oneshot(Request::post("/api/sync").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn trigger_sync_without_configured_context_returns_503() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let state = base_state(pool, None);
    let router = app(state, None);

    let response = router
        .oneshot(
            Request::post("/api/sync")
                .header("cookie", format!("session_id={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn trigger_sync_runs_and_returns_a_summary() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;

    sqlx::query(
        "INSERT INTO evenement (nom, date_debut, date_fin, sheet_id, bloc_formulaire) \
         VALUES ('Indiv', '2027-01-01', '2027-01-02', 'a-sheet', 'form_2027_indiv')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let token_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "fake-access-token",
            "expires_in": 3600,
        })))
        .mount(&token_server)
        .await;
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/sheets_responses_form1.json").unwrap(),
    )
    .unwrap();
    let header_only = serde_json::json!({ "values": [fixture["values"][0].clone()] });

    let sheets_server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(header_only))
        .mount(&sheets_server)
        .await;

    let auth = GoogleAuth::new(ServiceAccountKey {
        client_email: "test@example.iam.gserviceaccount.com".to_string(),
        private_key: TEST_PRIVATE_KEY.to_string(),
        token_uri: format!("{}/token", token_server.uri()),
    });
    let sync = SyncContext {
        auth,
        sheets: SheetsClient::new(sheets_server.uri()),
        running: Arc::new(Mutex::new(())),
    };

    let state = base_state(pool, Some(sync));
    let router = app(state, None);

    let response = router
        .oneshot(
            Request::post("/api/sync")
                .header("cookie", format!("session_id={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["sheets"][0]["sheet_id"], "a-sheet");
    assert_eq!(json["sheets"][0]["status"], "ok");
}
