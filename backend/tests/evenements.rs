mod common;

use benefrance_backend::evenements::{
    create_evenement, set_archive, update_evenement, EvenementInput, SaveEvenementError,
    SheetIdError,
};
use common::spawn_test_db;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn insert_admin(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO admins (email) VALUES ('admin@example.org') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn input(nom: &str, sheet_id: Option<&str>) -> EvenementInput {
    EvenementInput {
        nom: nom.to_string(),
        date_debut: chrono::NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
        date_fin: chrono::NaiveDate::from_ymd_opt(2027, 1, 2).unwrap(),
        lieu: Some("Poitiers".to_string()),
        // A known bloc whenever sheet_id is set, so tests that only care
        // about sheet_id validation don't also have to think about bloc
        // validation (covered separately below).
        bloc_formulaire: sheet_id.map(|_| "form_2027_indiv".to_string()),
        sheet_id: sheet_id.map(str::to_string),
    }
}

#[tokio::test]
async fn creates_an_event_without_sheet_id_with_no_validation_call() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let id = create_evenement(&pool, None, admin_id, input("Indiv", None))
        .await
        .unwrap();

    let nom: String = sqlx::query_scalar("SELECT nom FROM evenement WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(nom, "Indiv");
}

#[tokio::test]
async fn rejects_date_fin_before_date_debut() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let mut bad = input("Indiv", None);
    bad.date_fin = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

    let err = create_evenement(&pool, None, admin_id, bad)
        .await
        .unwrap_err();
    assert!(matches!(err, SaveEvenementError::InvalidDates));
}

#[tokio::test]
async fn sheet_id_validation_is_skipped_without_a_sync_context() {
    // No SyncContext (local dev / e2e without Google credentials, cf.
    // AppState.sync): a sheet_id is still accepted rather than blocking
    // event creation in every environment lacking real Google credentials.
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let id = create_evenement(&pool, None, admin_id, input("Indiv", Some("some-sheet-id")))
        .await
        .unwrap();

    let sheet_id: Option<String> =
        sqlx::query_scalar("SELECT sheet_id FROM evenement WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sheet_id.as_deref(), Some("some-sheet-id"));
}

async fn sync_context_against(
    sheets_status: u16,
) -> (MockServer, MockServer, benefrance_backend::SyncContext) {
    use benefrance_backend::google_auth::{GoogleAuth, ServiceAccountKey};
    use benefrance_backend::sheets::SheetsClient;

    const TEST_PRIVATE_KEY: &str = include_str!("fixtures/test_rsa_private_key.pem");

    let token_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "fake-access-token",
            "expires_in": 3600,
        })))
        .mount(&token_server)
        .await;

    let sheets_server = MockServer::start().await;
    let response = if sheets_status == 200 {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({ "values": [] }))
    } else {
        ResponseTemplate::new(sheets_status)
    };
    Mock::given(method("GET"))
        .respond_with(response)
        .mount(&sheets_server)
        .await;

    let auth = GoogleAuth::new(ServiceAccountKey {
        client_email: "test@example.iam.gserviceaccount.com".to_string(),
        private_key: TEST_PRIVATE_KEY.to_string(),
        token_uri: format!("{}/token", token_server.uri()),
    });
    let sync = benefrance_backend::SyncContext {
        auth,
        sheets: SheetsClient::new(sheets_server.uri()),
        running: std::sync::Arc::new(tokio::sync::Mutex::new(())),
    };
    (token_server, sheets_server, sync)
}

#[tokio::test]
async fn sheet_id_not_found_is_rejected_with_a_distinct_message() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let (_token, _sheets, sync) = sync_context_against(404).await;

    let err = create_evenement(
        &pool,
        Some(&sync),
        admin_id,
        input("Indiv", Some("missing")),
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err,
        SaveEvenementError::SheetId(SheetIdError::NotFound)
    ));
}

#[tokio::test]
async fn sheet_id_forbidden_is_rejected_with_a_distinct_message() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let (_token, _sheets, sync) = sync_context_against(403).await;

    let err = create_evenement(
        &pool,
        Some(&sync),
        admin_id,
        input("Indiv", Some("private")),
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err,
        SaveEvenementError::SheetId(SheetIdError::Forbidden)
    ));
}

#[tokio::test]
async fn sheet_id_accessible_is_accepted() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let (_token, _sheets, sync) = sync_context_against(200).await;

    create_evenement(
        &pool,
        Some(&sync),
        admin_id,
        input("Indiv", Some("ok-sheet")),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn update_changes_fields_and_archive_toggles_independently() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;
    let id = create_evenement(&pool, None, admin_id, input("Indiv", None))
        .await
        .unwrap();

    let mut renamed = input("Indiv renommé", None);
    renamed.lieu = Some("Futuroscope".to_string());
    update_evenement(&pool, None, admin_id, id, renamed)
        .await
        .unwrap();

    let (nom, lieu): (String, Option<String>) =
        sqlx::query_as("SELECT nom, lieu FROM evenement WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(nom, "Indiv renommé");
    assert_eq!(lieu.as_deref(), Some("Futuroscope"));

    set_archive(&pool, admin_id, id, true).await.unwrap();
    let archived: bool = sqlx::query_scalar("SELECT archive FROM evenement WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(archived);

    set_archive(&pool, admin_id, id, false).await.unwrap();
    let archived: bool = sqlx::query_scalar("SELECT archive FROM evenement WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !archived,
        "unarchiving must be possible and leave the event editable"
    );
}

#[tokio::test]
async fn empty_nom_is_rejected() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let err = create_evenement(&pool, None, admin_id, input("   ", None))
        .await
        .unwrap_err();
    assert!(matches!(err, SaveEvenementError::EmptyNom));
}

#[tokio::test]
async fn sheet_id_without_bloc_is_rejected() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let mut bad = input("Indiv", Some("some-sheet"));
    bad.bloc_formulaire = None;

    let err = create_evenement(&pool, None, admin_id, bad)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        SaveEvenementError::Bloc(benefrance_backend::evenements::BlocError::Missing)
    ));
}

#[tokio::test]
async fn unknown_bloc_is_rejected() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let mut bad = input("Indiv", Some("some-sheet"));
    bad.bloc_formulaire = Some("form_2099_made_up".to_string());

    let err = create_evenement(&pool, None, admin_id, bad)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        SaveEvenementError::Bloc(benefrance_backend::evenements::BlocError::Unknown(_))
    ));
}

#[tokio::test]
async fn two_events_cannot_claim_the_same_bloc_on_the_same_sheet() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    create_evenement(
        &pool,
        None,
        admin_id,
        input("Indiv 1", Some("shared-sheet")),
    )
    .await
    .unwrap();

    let err = create_evenement(
        &pool,
        None,
        admin_id,
        input("Indiv 2", Some("shared-sheet")),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        SaveEvenementError::Bloc(benefrance_backend::evenements::BlocError::AlreadyUsed(_))
    ));
}

#[tokio::test]
async fn updating_an_unknown_id_is_not_found() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let err = update_evenement(&pool, None, admin_id, Uuid::new_v4(), input("Indiv", None))
        .await
        .unwrap_err();
    assert!(matches!(err, SaveEvenementError::NotFound));
}

#[tokio::test]
async fn archiving_an_unknown_id_is_not_found() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let err = set_archive(&pool, admin_id, Uuid::new_v4(), true)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        benefrance_backend::evenements::SetArchiveError::NotFound
    ));
}

#[tokio::test]
async fn unarchiving_into_a_bloc_conflict_is_rejected() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool).await;

    let archived_id = create_evenement(
        &pool,
        None,
        admin_id,
        input("Indiv archivé", Some("shared-sheet")),
    )
    .await
    .unwrap();
    set_archive(&pool, admin_id, archived_id, true)
        .await
        .unwrap();

    // A new event takes the same bloc while the first is archived — legal.
    create_evenement(
        &pool,
        None,
        admin_id,
        input("Indiv actif", Some("shared-sheet")),
    )
    .await
    .unwrap();

    // Unarchiving the first must not silently recreate the conflict.
    let err = set_archive(&pool, admin_id, archived_id, false)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        benefrance_backend::evenements::SetArchiveError::Bloc(_)
    ));
}
