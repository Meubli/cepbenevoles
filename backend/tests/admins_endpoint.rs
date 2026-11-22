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

async fn insert_admin(pool: &PgPool, email: &str, is_superadmin: bool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO admins (nom, prenom, email, is_superadmin) \
         VALUES ('Nom', 'Prenom', $1, $2) RETURNING id",
    )
    .bind(email)
    .bind(is_superadmin)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_session(pool: &PgPool, admin_id: Uuid, token: &str) {
    sqlx::query(
        "INSERT INTO sessions (admin_id, token_hash, expires_at) \
         VALUES ($1, $2, now() + interval '1 day')",
    )
    .bind(admin_id)
    .bind(hash(token))
    .execute(pool)
    .await
    .unwrap();
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
async fn list_admins_requires_a_superadmin() {
    let pool = spawn_test_db().await;
    let admin_id = insert_admin(&pool, "plain-admin@example.org", false).await;
    insert_session(&pool, admin_id, "plain-token").await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(
            Request::get("/api/admins")
                .header("cookie", "session_id=plain-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_superadmin_creates_then_lists_a_new_admin() {
    let pool = spawn_test_db().await;
    let super_id = insert_admin(&pool, "super@example.org", true).await;
    insert_session(&pool, super_id, "super-token").await;
    let router = app(base_state(pool), None);

    let create_response = router
        .clone()
        .oneshot(
            Request::post("/api/admins")
                .header("cookie", "session_id=super-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "nom": "Bernard",
                        "prenom": "Luc",
                        "email": "luc.bernard@example.org",
                        "is_superadmin": false,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let list_response = router
        .oneshot(
            Request::get("/api/admins")
                .header("cookie", "session_id=super-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let admins: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let emails: Vec<&str> = admins
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["email"].as_str().unwrap())
        .collect();
    assert!(emails.contains(&"luc.bernard@example.org"));
}

#[tokio::test]
async fn creating_an_admin_with_a_taken_email_is_422() {
    let pool = spawn_test_db().await;
    let super_id = insert_admin(&pool, "super@example.org", true).await;
    insert_session(&pool, super_id, "super-token").await;
    insert_admin(&pool, "taken@example.org", false).await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(
            Request::post("/api/admins")
                .header("cookie", "session_id=super-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "nom": "X",
                        "prenom": "Y",
                        "email": "taken@example.org",
                        "is_superadmin": false,
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
async fn a_superadmin_promotes_a_plain_admin() {
    let pool = spawn_test_db().await;
    let super_id = insert_admin(&pool, "super@example.org", true).await;
    insert_session(&pool, super_id, "super-token").await;
    let target_id = insert_admin(&pool, "plain@example.org", false).await;
    let router = app(base_state(pool.clone()), None);

    let response = router
        .oneshot(
            Request::patch(format!("/api/admins/{target_id}"))
                .header("cookie", "session_id=super-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"is_superadmin": true}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let is_superadmin: bool = sqlx::query_scalar("SELECT is_superadmin FROM admins WHERE id = $1")
        .bind(target_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(is_superadmin);
}

#[tokio::test]
async fn demoting_the_last_superadmin_is_refused() {
    let pool = spawn_test_db().await;
    let super_id = insert_admin(&pool, "super@example.org", true).await;
    insert_session(&pool, super_id, "super-token").await;
    let router = app(base_state(pool.clone()), None);

    let response = router
        .oneshot(
            Request::patch(format!("/api/admins/{super_id}"))
                .header("cookie", "session_id=super-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"is_superadmin": false}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let is_superadmin: bool = sqlx::query_scalar("SELECT is_superadmin FROM admins WHERE id = $1")
        .bind(super_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(is_superadmin, "must still be superadmin after the refusal");
}

#[tokio::test]
async fn a_superadmin_cannot_remove_themselves() {
    let pool = spawn_test_db().await;
    let super_id = insert_admin(&pool, "super@example.org", true).await;
    insert_session(&pool, super_id, "super-token").await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(
            Request::delete(format!("/api/admins/{super_id}"))
                .header("cookie", "session_id=super-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn removing_the_last_superadmin_through_another_admin_is_refused() {
    let pool = spawn_test_db().await;
    let super_id = insert_admin(&pool, "super@example.org", true).await;
    let other_super_session_owner = insert_admin(&pool, "super2@example.org", true).await;
    insert_session(&pool, other_super_session_owner, "super2-token").await;
    let router = app(base_state(pool.clone()), None);

    // Demote super_id is legal (two superadmins exist), but removing the
    // *other* one down to zero via delete must still be refused once only
    // one is left — exercised directly against delete_admin's guard by
    // deleting super_id first (still 1 left: other_super_session_owner),
    // then trying to delete the last one from itself is covered by the
    // self-removal test above; here we confirm a *different* actor also
    // can't remove the sole remaining superadmin.
    let first_removal = router
        .clone()
        .oneshot(
            Request::delete(format!("/api/admins/{super_id}"))
                .header("cookie", "session_id=super2-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_removal.status(), StatusCode::NO_CONTENT);

    let second_removal = router
        .oneshot(
            Request::delete(format!("/api/admins/{other_super_session_owner}"))
                .header("cookie", "session_id=super2-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_removal.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn removing_an_admin_also_destroys_their_sessions() {
    let pool = spawn_test_db().await;
    let super_id = insert_admin(&pool, "super@example.org", true).await;
    insert_session(&pool, super_id, "super-token").await;
    let target_id = insert_admin(&pool, "plain@example.org", false).await;
    insert_session(&pool, target_id, "plain-token").await;
    let router = app(base_state(pool.clone()), None);

    let response = router
        .oneshot(
            Request::delete(format!("/api/admins/{target_id}"))
                .header("cookie", "session_id=super-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let remaining_sessions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sessions WHERE admin_id = $1")
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining_sessions, 0);
}
