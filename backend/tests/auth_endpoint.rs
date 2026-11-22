mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use benefrance_backend::{app, AppState};
use chrono::{Duration, Utc};
use common::spawn_test_db;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

async fn insert_session(pool: &PgPool, email: &str, token: &str) -> Uuid {
    let admin_id: Uuid = sqlx::query_scalar("INSERT INTO admins (email) VALUES ($1) RETURNING id")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (admin_id, token_hash, expires_at) \
         VALUES ($1, $2, now() + interval '1 day')",
    )
    .bind(admin_id)
    .bind(hash(token))
    .execute(pool)
    .await
    .unwrap();
    admin_id
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
    }
}

#[tokio::test]
async fn session_check_returns_the_admin_identity() {
    let pool = spawn_test_db().await;
    insert_session(&pool, "admin@example.org", "a-token").await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(
            Request::get("/api/session")
                .header("cookie", "session_id=a-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["email"], "admin@example.org");
    assert_eq!(parsed["is_superadmin"], false);
}

#[tokio::test]
async fn logout_destroys_the_session_and_clears_the_cookie() {
    let pool = spawn_test_db().await;
    insert_session(&pool, "admin@example.org", "a-token").await;
    let router = app(base_state(pool.clone()), None);

    let logout_response = router
        .clone()
        .oneshot(
            Request::post("/api/auth/logout")
                .header("cookie", "session_id=a-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0, "the session row must be gone from the DB");

    let after_logout = router
        .oneshot(
            Request::get("/api/session")
                .header("cookie", "session_id=a-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after_logout.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_without_a_session_cookie_is_a_no_op_not_an_error() {
    let pool = spawn_test_db().await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(
            Request::post("/api/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_valid_request_slides_the_session_expiration_forward() {
    let pool = spawn_test_db().await;
    let admin_id = insert_session(&pool, "admin@example.org", "a-token").await;

    // Back-date the session close to its original 1-day expiry, as if it
    // had been created a while ago, so a renewal is actually observable.
    let near_expiry = Utc::now() + Duration::minutes(5);
    sqlx::query("UPDATE sessions SET expires_at = $1 WHERE admin_id = $2")
        .bind(near_expiry)
        .bind(admin_id)
        .execute(&pool)
        .await
        .unwrap();

    let router = app(base_state(pool.clone()), None);
    let response = router
        .oneshot(
            Request::get("/api/session")
                .header("cookie", "session_id=a-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let new_expires_at: chrono::DateTime<Utc> =
        sqlx::query_scalar("SELECT expires_at FROM sessions WHERE admin_id = $1")
            .bind(admin_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert!(
        new_expires_at > near_expiry + Duration::days(170),
        "expires_at should have slid forward to ~180 days out, got {new_expires_at}"
    );
}

#[tokio::test]
async fn an_already_expired_session_is_rejected_not_renewed() {
    let pool = spawn_test_db().await;
    let admin_id = insert_session(&pool, "admin@example.org", "a-token").await;

    let expired = Utc::now() - Duration::minutes(1);
    sqlx::query("UPDATE sessions SET expires_at = $1 WHERE admin_id = $2")
        .bind(expired)
        .bind(admin_id)
        .execute(&pool)
        .await
        .unwrap();

    let router = app(base_state(pool.clone()), None);
    let response = router
        .oneshot(
            Request::get("/api/session")
                .header("cookie", "session_id=a-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let still_expired: chrono::DateTime<Utc> =
        sqlx::query_scalar("SELECT expires_at FROM sessions WHERE admin_id = $1")
            .bind(admin_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        (still_expired - expired).num_milliseconds().abs() < 1,
        "an expired session must not be silently resurrected: expected ~{expired}, got {still_expired}"
    );
}
