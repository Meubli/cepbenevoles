mod common;

use std::collections::HashMap;

use benefrance_backend::sync::form_2027::parse_row;
use benefrance_backend::sync::persist::persist_parsed_rows;
use benefrance_backend::sync::{EventSlug, ParsedRow};
use common::spawn_test_db;
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn fixture_rows() -> Vec<ParsedRow> {
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/sheets_responses_form1.json")).unwrap();
    let values = raw["values"].as_array().unwrap().clone();
    let header: Vec<String> = values[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    values[1..]
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let row: Vec<String> = row
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            parse_row(&header, &row, i + 1).expect("fixture rows must parse")
        })
        .collect()
}

async fn insert_evenement(
    pool: &PgPool,
    nom: &str,
    date_debut: &str,
    date_fin: &str,
    bloc_formulaire: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO evenement (nom, date_debut, date_fin, sheet_id, bloc_formulaire) \
         VALUES ($1, $2, $3, 'fake-sheet', $4) \
         RETURNING id",
    )
    .bind(nom)
    .bind(date_debut.parse::<chrono::NaiveDate>().unwrap())
    .bind(date_fin.parse::<chrono::NaiveDate>().unwrap())
    .bind(bloc_formulaire)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_events(pool: &PgPool) -> HashMap<EventSlug, Uuid> {
    let indiv_id = insert_evenement(
        pool,
        "France Indiv Perf GAM/GAF",
        "2027-04-29",
        "2027-05-04",
        "form_2027_indiv",
    )
    .await;
    let teamgym_id = insert_evenement(
        pool,
        "France TeamGym Performance",
        "2027-06-11",
        "2027-06-15",
        "form_2027_teamgym",
    )
    .await;

    HashMap::from([
        (EventSlug::IndivGamGaf, indiv_id),
        (EventSlug::TeamGym, teamgym_id),
    ])
}

#[tokio::test]
async fn persists_one_soumission_per_attended_event() {
    let pool = spawn_test_db().await;
    let routing = seed_events(&pool).await;
    let rows = fixture_rows();

    let report = persist_parsed_rows(&pool, &routing, &rows).await.unwrap();

    // Martin (Indiv only) + Dubois (Indiv + TeamGym) + Zoé (Indiv only)
    // = 4 soumissions from 3 rows.
    assert_eq!(report.inserted, 4);
    assert_eq!(report.already_present, 0);
    assert_eq!(report.unrouted, 0);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM soumission")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 4);
}

#[tokio::test]
async fn resyncing_the_same_rows_inserts_nothing_new() {
    let pool = spawn_test_db().await;
    let routing = seed_events(&pool).await;
    let rows = fixture_rows();

    persist_parsed_rows(&pool, &routing, &rows).await.unwrap();
    let second = persist_parsed_rows(&pool, &routing, &rows).await.unwrap();

    assert_eq!(second.inserted, 0);
    assert_eq!(second.already_present, 4);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM soumission")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 4, "resync must not duplicate rows");
}

#[tokio::test]
async fn attendance_for_an_unrouted_event_is_skipped_not_fatal() {
    let pool = spawn_test_db().await;
    // Only route Indiv — TeamGym is left unmapped, as if the admin hadn't
    // configured that event's bloc_formulaire yet.
    let indiv_id = insert_evenement(
        &pool,
        "France Indiv Perf GAM/GAF",
        "2027-04-29",
        "2027-05-04",
        "form_2027_indiv",
    )
    .await;
    let routing = HashMap::from([(EventSlug::IndivGamGaf, indiv_id)]);
    let rows = fixture_rows();

    let report = persist_parsed_rows(&pool, &routing, &rows).await.unwrap();

    // Dubois's TeamGym attendance has nowhere to go.
    assert_eq!(report.unrouted, 1);
    assert_eq!(report.inserted, 3, "the 3 Indiv attendances still persist");
}

#[tokio::test]
async fn persists_availabilities_and_raw_wish_labels() {
    let pool = spawn_test_db().await;
    let routing = seed_events(&pool).await;
    let rows = fixture_rows();

    persist_parsed_rows(&pool, &routing, &rows).await.unwrap();

    let martin_wish: String = sqlx::query(
        "SELECT libelle_brut FROM soumission_libelle sl \
         JOIN soumission s ON s.id = sl.soumission_id \
         WHERE s.nom = 'Martin'",
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .get(0);
    assert_eq!(martin_wish, "Transport");

    let dubois_indiv_slots: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM disponibilite_soumission ds \
         JOIN soumission s ON s.id = ds.soumission_id \
         WHERE s.nom = 'Dubois' AND s.evenement_id = $1",
    )
    .bind(routing[&EventSlug::IndivGamGaf])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(dubois_indiv_slots > 0);
}
