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

/// Real form structure, anonymized sample rows — captured from the actual
/// production Sheet (cf. specs.md §3.1) so parsing code is exercised against
/// the real header layout and the real "trailing empty cells are omitted"
/// quirk of the Sheets API, not an idealized fixture.
#[tokio::test]
async fn sheets_client_parses_the_real_form_structure_fixture() {
    let mock_server = MockServer::start().await;
    let fixture = include_str!("fixtures/sheets_responses_form1.json");

    Mock::given(method("GET"))
        .and(path(
            "/v4/spreadsheets/1VanRtLnS2lF5XpUtUpjyv9MX2lQh7EPcAZCIPsm3jsc/values/Reponses!A1:AB4",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture, "application/json"))
        .mount(&mock_server)
        .await;

    let client = SheetsClient::new(mock_server.uri(), "fake-token");

    let rows = client
        .fetch_values(
            "1VanRtLnS2lF5XpUtUpjyv9MX2lQh7EPcAZCIPsm3jsc",
            "Reponses!A1:AB4",
        )
        .await
        .expect("fetch against the real-shaped fixture should succeed");

    assert_eq!(rows.len(), 4, "header + 3 anonymized submission rows");

    let header = &rows[0];
    assert_eq!(header.len(), 28, "real form has 28 columns");
    assert_eq!(header[0], "Horodateur");
    assert_eq!(header[1], "Nom");

    // The Sheets API omits trailing empty cells per row, so data rows are
    // shorter than the header whenever trailing columns are unanswered —
    // any future column-indexed parsing must not assume `row.len() ==
    // header.len()` and must pad/index defensively.
    for data_row in &rows[1..] {
        assert!(
            data_row.len() < header.len(),
            "fixture rows keep the real API's trailing-empty-cell truncation"
        );
    }

    assert_eq!(
        rows[1][1], "Martin",
        "anonymized, not the real volunteer's name"
    );
    assert_eq!(
        rows[3][3], "13 - 14 ans",
        "age bracket column matches real form wording"
    );
}
