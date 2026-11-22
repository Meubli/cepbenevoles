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

#[tokio::test]
async fn create_then_list_then_archive_roundtrip() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let create_response = router
        .clone()
        .oneshot(
            Request::post("/api/evenements")
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "nom": "France Indiv",
                        "date_debut": "2027-04-29",
                        "date_fin": "2027-05-04",
                        "lieu": "Poitiers",
                        "sheet_id": null,
                        "bloc_formulaire": null,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    let list_response = router
        .clone()
        .oneshot(
            Request::get("/api/evenements")
                .header("cookie", format!("session_id={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let event = list
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap();
    assert_eq!(event["archive"], false);

    let archive_response = router
        .oneshot(
            Request::patch(format!("/api/evenements/{id}/archive"))
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({"archive": true}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(archive_response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn invalid_dates_is_422() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(
            Request::post("/api/evenements")
                .header("cookie", format!("session_id={token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "nom": "Invalide",
                        "date_debut": "2027-05-04",
                        "date_fin": "2027-04-29",
                        "lieu": null,
                        "sheet_id": null,
                        "bloc_formulaire": null,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn list_evenements_requires_a_session() {
    let pool = spawn_test_db().await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(Request::get("/api/evenements").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
