mod common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use benefrance_backend::{app, live, AppState};
use common::spawn_test_db;
use serde_json::json;
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

fn base_state(pool: PgPool, live: live::LiveSender) -> AppState {
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
        live,
    }
}

#[tokio::test]
async fn ws_requires_a_session() {
    let pool = spawn_test_db().await;
    let router = app(base_state(pool, live::channel()), None);

    let response = router
        .oneshot(Request::get("/api/ws").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_successful_write_broadcasts_its_resource_tag() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let sender = live::channel();
    let mut rx = sender.subscribe();
    let router = app(base_state(pool, sender), None);

    let body = json!({
        "nom": "Sécurité",
        "prerequis_libre_texte": null,
        "a_des_postes": false,
        "grille_creneaux": "generale",
        "besoin_chiffre": true,
        "couleur": "#5ec9a0",
    });

    let response = router
        .oneshot(
            Request::post("/api/commissions")
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("a resource tag should have been broadcast")
        .unwrap();
    assert_eq!(event.tag, "commissions");
    assert_eq!(event.origin, None);
}

#[tokio::test]
async fn a_client_id_header_is_relayed_as_the_event_origin() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let sender = live::channel();
    let mut rx = sender.subscribe();
    let router = app(base_state(pool, sender), None);

    let client_id = Uuid::new_v4();
    let response = router
        .oneshot(
            Request::post("/api/commissions")
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .header("x-client-id", client_id.to_string())
                .body(Body::from(commission_body("Sécurité").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.origin, Some(client_id));
}

#[tokio::test]
async fn a_malformed_client_id_header_is_ignored_not_rejected() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let sender = live::channel();
    let mut rx = sender.subscribe();
    let router = app(base_state(pool, sender), None);

    let response = router
        .oneshot(
            Request::post("/api/commissions")
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .header("x-client-id", "not-a-uuid")
                .body(Body::from(commission_body("Sécurité").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.origin, None);
}

fn commission_body(nom: &str) -> serde_json::Value {
    json!({
        "nom": nom,
        "prerequis_libre_texte": null,
        "a_des_postes": false,
        "grille_creneaux": "generale",
        "besoin_chiffre": true,
        "couleur": "#5ec9a0",
    })
}

#[tokio::test]
async fn a_failed_write_does_not_broadcast() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let sender = live::channel();
    let rx = sender.subscribe();
    let router = app(base_state(pool, sender), None);

    // Empty nom is rejected (422) — see commissions_endpoint.rs.
    let body = json!({
        "nom": "   ",
        "prerequis_libre_texte": null,
        "a_des_postes": false,
        "grille_creneaux": "generale",
        "besoin_chiffre": true,
        "couleur": "#5ec9a0",
    });

    let response = router
        .oneshot(
            Request::post("/api/commissions")
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    assert_no_tag_broadcast(rx).await;
}

#[tokio::test]
async fn a_get_request_does_not_broadcast() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let sender = live::channel();
    let rx = sender.subscribe();
    let router = app(base_state(pool, sender), None);

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

    assert_no_tag_broadcast(rx).await;
}

/// Asserts no actual resource tag arrives. The channel closing when its
/// sender (held by the by-then-dropped `router`/`AppState`) goes away —
/// which `recv()` also surfaces promptly as `Err(RecvError::Closed)`, same
/// shape as a real send — is not itself evidence of a broadcast: only an
/// `Ok(tag)` is.
async fn assert_no_tag_broadcast(mut rx: tokio::sync::broadcast::Receiver<live::LiveEvent>) {
    if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await {
        panic!("unexpected broadcast: {}", event.tag);
    }
}
