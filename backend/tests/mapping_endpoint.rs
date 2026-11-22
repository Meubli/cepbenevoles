mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use benefrance_backend::{app, AppState};
use common::spawn_test_db;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

async fn insert_session(pool: &PgPool) -> String {
    let admin_id: Uuid =
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

fn base_state(pool: PgPool) -> AppState {
    AppState {
        db: pool,
        smtp_host: "localhost".to_string(),
        smtp_port: 2525,
        smtp_user: None,
        smtp_pass: None,
        smtp_from: "noreply@example.org".to_string(),
        sync: None,
        session_cookie_secure: true,
        sheet_validation_required: true,
        live: benefrance_backend::live::channel(),
    }
}

async fn insert_evenement(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO evenement (nom, date_debut, date_fin) \
         VALUES ('Indiv', '2027-01-01', '2027-01-02') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_soumission_with_libelle(pool: &PgPool, evenement_id: Uuid, libelle: &str) {
    let soumission_id: Uuid = sqlx::query_scalar(
        "INSERT INTO soumission \
            (evenement_id, nom, prenom, email, telephone, tranche_age, permis_b, taille_tshirt, date_soumission) \
         VALUES ($1, 'Nom', 'Prenom', 'a@example.com', '0600000000', '18 ans et +', true, 'M', now()) \
         RETURNING id",
    )
    .bind(evenement_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO soumission_libelle (soumission_id, libelle_brut) VALUES ($1, $2)")
        .bind(soumission_id)
        .bind(libelle)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn list_mapping_requires_a_session() {
    let pool = spawn_test_db().await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(Request::get("/api/mapping").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_mapping_shows_an_unmapped_label_and_a_mapped_one() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let evenement_id = insert_evenement(&pool).await;
    insert_soumission_with_libelle(&pool, evenement_id, "Un libellé jamais vu").await;
    insert_soumission_with_libelle(&pool, evenement_id, "Accueil").await;

    let router = app(base_state(pool), None);
    let response = router
        .oneshot(
            Request::get("/api/mapping")
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
    let rows = json.as_array().unwrap();

    let unmapped = rows
        .iter()
        .find(|r| r["libelle"] == "Un libellé jamais vu")
        .unwrap();
    assert!(unmapped["kind"].is_null());

    let mapped = rows.iter().find(|r| r["libelle"] == "Accueil").unwrap();
    assert_eq!(mapped["commission_nom"], "Accueil");
}

#[tokio::test]
async fn post_mapping_resolves_the_label_and_decrements_unmapped_count() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let evenement_id = insert_evenement(&pool).await;
    insert_soumission_with_libelle(&pool, evenement_id, "Sécurité / SSIAP").await;

    let accueil_id: Uuid = sqlx::query_scalar("SELECT id FROM commission WHERE nom = 'Accueil'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let router = app(base_state(pool.clone()), None);
    let response = router
        .oneshot(
            Request::post("/api/mapping")
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "libelle": "Sécurité / SSIAP",
                        "kind": "commission",
                        "commission_id": accueil_id,
                        "poste_id": null,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let resolved: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT resolved_at FROM soumission_libelle WHERE libelle_brut = 'Sécurité / SSIAP'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(resolved.is_some());
}

#[tokio::test]
async fn post_mapping_for_an_unseen_label_is_404() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let accueil_id: Uuid = sqlx::query_scalar("SELECT id FROM commission WHERE nom = 'Accueil'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let router = app(base_state(pool), None);
    let response = router
        .oneshot(
            Request::post("/api/mapping")
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "libelle": "Jamais vu nulle part",
                        "kind": "commission",
                        "commission_id": accueil_id,
                        "poste_id": null,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_commissions_returns_the_seeded_referential_with_nested_postes() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;

    let router = app(base_state(pool), None);
    let response = router
        .oneshot(
            Request::get("/api/commissions")
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
    let commissions = json.as_array().unwrap();

    let plateau = commissions.iter().find(|c| c["nom"] == "Plateau").unwrap();
    let postes = plateau["postes"].as_array().unwrap();
    assert!(postes.iter().any(|p| p["nom"] == "Juge de ligne"));
    assert_eq!(plateau["couleur"], "#9065c9");
}
