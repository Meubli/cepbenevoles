mod common;

use benefrance_backend::google_auth::{GoogleAuth, ServiceAccountKey};
use benefrance_backend::sheets::SheetsClient;
use benefrance_backend::sync::orchestrate::{sync_all, sync_all_and_record, SheetSyncError};
use common::spawn_test_db;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_PRIVATE_KEY: &str = include_str!("fixtures/test_rsa_private_key.pem");

async fn mock_token_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "fake-access-token",
            "expires_in": 3600,
        })))
        .mount(&server)
        .await;
    server
}

fn test_key(token_uri: String) -> ServiceAccountKey {
    ServiceAccountKey {
        client_email: "test@example.iam.gserviceaccount.com".to_string(),
        private_key: TEST_PRIVATE_KEY.to_string(),
        token_uri,
    }
}

async fn insert_evenement(
    pool: &PgPool,
    nom: &str,
    sheet_id: &str,
    bloc_formulaire: Option<&str>,
    archive: bool,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO evenement (nom, date_debut, date_fin, sheet_id, bloc_formulaire, archive) \
         VALUES ($1, '2027-01-01', '2027-01-02', $2, $3, $4) \
         RETURNING id",
    )
    .bind(nom)
    .bind(sheet_id)
    .bind(bloc_formulaire)
    .bind(archive)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn sheets_values_response(fixture_path: &str, tab_name_encoded: &str) -> (String, String) {
    (
        format!("/v4/spreadsheets/.*/values/{tab_name_encoded}"),
        std::fs::read_to_string(fixture_path).unwrap(),
    )
}

#[tokio::test]
async fn two_events_sharing_one_sheet_fetch_it_once_and_insert_four_submissions() {
    let pool = spawn_test_db().await;
    let token_server = mock_token_server().await;
    let sheets_server = MockServer::start().await;

    let (path_pattern, body) = sheets_values_response(
        "tests/fixtures/sheets_responses_form1.json",
        "R%C3%A9ponses%20au%20formulaire%201!A1:ZZ",
    );
    Mock::given(method("GET"))
        .and(path_regex(path_pattern))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .expect(1) // fetched once even though 2 events share this sheet_id
        .mount(&sheets_server)
        .await;

    insert_evenement(
        &pool,
        "Indiv",
        "shared-sheet",
        Some("form_2027_indiv"),
        false,
    )
    .await;
    insert_evenement(
        &pool,
        "TeamGym",
        "shared-sheet",
        Some("form_2027_teamgym"),
        false,
    )
    .await;

    let auth = GoogleAuth::new(test_key(format!("{}/token", token_server.uri())));
    let sheets = SheetsClient::new(sheets_server.uri());

    let outcomes = sync_all(&pool, &auth, &sheets).await.unwrap();

    assert_eq!(outcomes.len(), 1);
    let (report, row_errors) = outcomes[0].result.as_ref().unwrap();
    assert_eq!(report.inserted, 4);
    assert!(row_errors.is_empty());

    sheets_server.verify().await;
}

#[tokio::test]
async fn one_sheet_failing_does_not_block_another() {
    let pool = spawn_test_db().await;
    let token_server = mock_token_server().await;
    let sheets_server = MockServer::start().await;

    let (ok_path, ok_body) = sheets_values_response(
        "tests/fixtures/sheets_responses_form1.json",
        "R%C3%A9ponses%20au%20formulaire%201!A1:ZZ",
    );

    Mock::given(method("GET"))
        .and(path_regex("/v4/spreadsheets/forbidden-sheet/.*"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&sheets_server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex("/v4/spreadsheets/ok-sheet/.*"))
        .and(path_regex(ok_path))
        .respond_with(ResponseTemplate::new(200).set_body_raw(ok_body, "application/json"))
        .mount(&sheets_server)
        .await;

    insert_evenement(
        &pool,
        "Forbidden",
        "forbidden-sheet",
        Some("form_2027_indiv"),
        false,
    )
    .await;
    insert_evenement(&pool, "Ok", "ok-sheet", Some("form_2027_indiv"), false).await;

    let auth = GoogleAuth::new(test_key(format!("{}/token", token_server.uri())));
    let sheets = SheetsClient::new(sheets_server.uri());

    let outcomes = sync_all(&pool, &auth, &sheets).await.unwrap();
    assert_eq!(outcomes.len(), 2);

    let forbidden = outcomes
        .iter()
        .find(|o| o.sheet_id == "forbidden-sheet")
        .unwrap();
    assert!(matches!(forbidden.result, Err(SheetSyncError::Sheets(_))));

    let ok = outcomes.iter().find(|o| o.sheet_id == "ok-sheet").unwrap();
    let (report, _) = ok.result.as_ref().unwrap();
    assert_eq!(
        report.inserted, 3,
        "only the Indiv attendances in this fixture"
    );
}

#[tokio::test]
async fn renamed_header_column_is_a_sheet_level_error() {
    let pool = spawn_test_db().await;
    let token_server = mock_token_server().await;
    let sheets_server = MockServer::start().await;

    let raw: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/sheets_responses_form1.json").unwrap(),
    )
    .unwrap();
    let mut values = raw["values"].clone();
    // Rename a required column, as the association has done before
    // (Déroulement -> Plateau).
    values[0][8] = serde_json::Value::String("Un intitulé renommé".to_string());

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": values,
        })))
        .mount(&sheets_server)
        .await;

    insert_evenement(
        &pool,
        "Indiv",
        "renamed-header-sheet",
        Some("form_2027_indiv"),
        false,
    )
    .await;

    let auth = GoogleAuth::new(test_key(format!("{}/token", token_server.uri())));
    let sheets = SheetsClient::new(sheets_server.uri());

    let outcomes = sync_all(&pool, &auth, &sheets).await.unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(matches!(
        outcomes[0].result,
        Err(SheetSyncError::InvalidHeader(_))
    ));
}

#[tokio::test]
async fn token_endpoint_failure_is_an_auth_error() {
    let pool = spawn_test_db().await;
    let token_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
        .mount(&token_server)
        .await;
    let sheets_server = MockServer::start().await;

    insert_evenement(&pool, "Indiv", "some-sheet", Some("form_2027_indiv"), false).await;

    let auth = GoogleAuth::new(test_key(format!("{}/token", token_server.uri())));
    let sheets = SheetsClient::new(sheets_server.uri());

    let outcomes = sync_all(&pool, &auth, &sheets).await.unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(matches!(outcomes[0].result, Err(SheetSyncError::Auth(_))));
}

#[tokio::test]
async fn archived_event_is_excluded_and_missing_bloc_is_reported() {
    let pool = spawn_test_db().await;
    let token_server = mock_token_server().await;
    let sheets_server = MockServer::start().await;

    insert_evenement(
        &pool,
        "Archived",
        "archived-sheet",
        Some("form_2027_indiv"),
        true,
    )
    .await;
    insert_evenement(&pool, "NoBloc", "no-bloc-sheet", None, false).await;

    let auth = GoogleAuth::new(test_key(format!("{}/token", token_server.uri())));
    let sheets = SheetsClient::new(sheets_server.uri());

    let outcomes = sync_all(&pool, &auth, &sheets).await.unwrap();

    assert!(
        outcomes.iter().all(|o| o.sheet_id != "archived-sheet"),
        "archived events must not be synced"
    );
    let no_bloc = outcomes
        .iter()
        .find(|o| o.sheet_id == "no-bloc-sheet")
        .unwrap();
    assert!(matches!(
        no_bloc.result,
        Err(SheetSyncError::MissingBloc { .. })
    ));
}

#[tokio::test]
async fn sync_all_and_record_writes_sync_run_and_row_errors() {
    let pool = spawn_test_db().await;
    let token_server = mock_token_server().await;
    let sheets_server = MockServer::start().await;

    let raw: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/sheets_responses_form1.json").unwrap(),
    )
    .unwrap();
    let mut values = raw["values"].clone();
    // Break one data row's horodateur so it produces a RowError alongside
    // the 3 that still parse fine.
    values[1][0] = serde_json::Value::String("not a date".to_string());

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": values,
        })))
        .mount(&sheets_server)
        .await;

    insert_evenement(
        &pool,
        "Indiv",
        "recorded-sheet",
        Some("form_2027_indiv"),
        false,
    )
    .await;

    let auth = GoogleAuth::new(test_key(format!("{}/token", token_server.uri())));
    let sheets = SheetsClient::new(sheets_server.uri());

    sync_all_and_record(&pool, &auth, &sheets).await.unwrap();

    let run_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM sync_run WHERE sheet_id = 'recorded-sheet' AND error IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let error_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sync_row_error WHERE sync_run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        error_count, 1,
        "the broken-horodateur row should be recorded"
    );
}

#[tokio::test]
async fn sync_all_and_record_captures_sheet_level_failure() {
    let pool = spawn_test_db().await;
    let token_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
        .mount(&token_server)
        .await;
    let sheets_server = MockServer::start().await;

    insert_evenement(
        &pool,
        "Indiv",
        "failing-sheet",
        Some("form_2027_indiv"),
        false,
    )
    .await;

    let auth = GoogleAuth::new(test_key(format!("{}/token", token_server.uri())));
    let sheets = SheetsClient::new(sheets_server.uri());

    sync_all_and_record(&pool, &auth, &sheets).await.unwrap();

    let error: Option<String> =
        sqlx::query_scalar("SELECT error FROM sync_run WHERE sheet_id = 'failing-sheet'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(error.is_some());
}
