mod common;

use benefrance_backend::sheets::SheetsClient;
use common::spawn_test_db;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn migrations_apply_and_health_query_runs() {
    let pool = spawn_test_db().await;

    let row: (i32,) = sqlx::query_as("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("health query should run against a migrated database");

    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn sheets_client_parses_a_fixture_row_from_the_mock_api() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v4/spreadsheets/fake-sheet-id/values/Reponses!A2:Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [
                ["Jean", "Dupont", "jean.dupont@example.com", "oui"]
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = SheetsClient::new(mock_server.uri(), "fake-token");

    let rows = client
        .fetch_values("fake-sheet-id", "Reponses!A2:Z")
        .await
        .expect("fetch against the mock should succeed");

    assert_eq!(
        rows,
        vec![vec![
            "Jean".to_string(),
            "Dupont".to_string(),
            "jean.dupont@example.com".to_string(),
            "oui".to_string(),
        ]]
    );
}
