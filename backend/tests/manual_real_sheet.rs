// Manual-only check against the real Google Sheet and real Google OAuth2
// endpoint — never run in CI (needs a real service-account key file and
// real network access). wiremock proves the request/parsing shape; this is
// the only thing that proves the JWT is actually accepted by Google and the
// PEM key parses correctly.
//
// Run with:
//   REAL_KEY_PATH=/path/to/key.json REAL_SHEET_ID=<id> \
//     cargo test --manifest-path backend/Cargo.toml --test manual_real_sheet -- --ignored --nocapture

use benefrance_backend::google_auth::{GoogleAuth, ServiceAccountKey};
use benefrance_backend::sheets::SheetsClient;
use benefrance_backend::sync::form_2027::TAB_NAME;

#[tokio::test]
#[ignore]
async fn real_token_and_fetch_against_live_sheet() {
    let key_path = std::env::var("REAL_KEY_PATH").expect("set REAL_KEY_PATH");
    let sheet_id = std::env::var("REAL_SHEET_ID").expect("set REAL_SHEET_ID");

    let key_json = std::fs::read_to_string(&key_path)
        .unwrap_or_else(|e| panic!("failed to read {key_path}: {e}"));
    let key = ServiceAccountKey::from_json(&key_json).expect("invalid service account JSON");

    let auth = GoogleAuth::new(key);
    let token = auth
        .access_token()
        .await
        .expect("real token request to Google failed");
    println!("token acquired, len={}", token.len());

    let sheets = SheetsClient::new("https://sheets.googleapis.com".to_string());
    let values = sheets
        .fetch_values(&token, &sheet_id, TAB_NAME)
        .await
        .expect("real fetch against the live Sheet failed");

    // Counts only — a data row is real volunteer PII (name, email, phone),
    // never printed here.
    println!("rows (incl. header): {}", values.len());
    assert!(!values.is_empty(), "expected at least the header row");
}
