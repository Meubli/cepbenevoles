mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use benefrance_backend::{app, AppState};
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

async fn request(
    router: axum::Router,
    method: &str,
    path: &str,
    token: &str,
    body: Option<serde_json::Value>,
) -> axum::http::Response<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", format!("session_id={token}"))
        .header("content-type", "application/json");
    let body = match body {
        Some(b) => Body::from(b.to_string()),
        None => Body::empty(),
    };
    router.oneshot(builder.body(body).unwrap()).await.unwrap()
}

#[tokio::test]
async fn list_commissions_requires_a_session() {
    let pool = spawn_test_db().await;
    let router = app(base_state(pool), None);

    let response = router
        .oneshot(
            Request::get("/api/commissions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn creates_then_lists_a_commission() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let response = request(
        router.clone(),
        "POST",
        "/api/commissions",
        &token,
        Some(commission_body("Sécurité")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = request(router, "GET", "/api/commissions", &token, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let commissions = json.as_array().unwrap();
    assert!(commissions.iter().any(|c| c["nom"] == "Sécurité"));
}

#[tokio::test]
async fn empty_nom_is_rejected() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let response = request(
        router,
        "POST",
        "/api/commissions",
        &token,
        Some(commission_body("   ")),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn too_long_nom_is_rejected() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let response = request(
        router,
        "POST",
        "/api/commissions",
        &token,
        Some(commission_body(&"a".repeat(101))),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn too_long_prerequis_is_rejected() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let mut body = commission_body("Sécurité");
    body["prerequis_libre_texte"] = json!("a".repeat(501));

    let response = request(router, "POST", "/api/commissions", &token, Some(body)).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn too_long_poste_nom_is_rejected() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let mut body = commission_body("Logistique");
    body["a_des_postes"] = json!(true);
    let response = request(
        router.clone(),
        "POST",
        "/api/commissions",
        &token,
        Some(body),
    )
    .await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let commission_id = json["id"].as_str().unwrap();

    let response = request(
        router,
        "POST",
        &format!("/api/commissions/{commission_id}/postes"),
        &token,
        Some(json!({"nom": "a".repeat(101)})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn duplicate_nom_is_rejected() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let response = request(
        router.clone(),
        "POST",
        "/api/commissions",
        &token,
        Some(commission_body("Sécurité")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = request(
        router,
        "POST",
        "/api/commissions",
        &token,
        Some(commission_body("Sécurité")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn invalid_couleur_is_rejected() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let mut body = commission_body("Sécurité");
    body["couleur"] = json!("not-a-color");

    let response = request(router, "POST", "/api/commissions", &token, Some(body)).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn updating_an_unknown_id_is_not_found() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let response = request(
        router,
        "PATCH",
        &format!("/api/commissions/{}", Uuid::new_v4()),
        &token,
        Some(commission_body("Sécurité")),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_changes_fields() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let response = request(
        router.clone(),
        "POST",
        "/api/commissions",
        &token,
        Some(commission_body("Sécurité")),
    )
    .await;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = json["id"].as_str().unwrap();

    let mut update_body = commission_body("Sécurité renommée");
    update_body["besoin_chiffre"] = json!(false);

    let response = request(
        router.clone(),
        "PATCH",
        &format!("/api/commissions/{id}"),
        &token,
        Some(update_body),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = request(router, "GET", "/api/commissions", &token, None).await;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let commissions = json.as_array().unwrap();
    let updated = commissions
        .iter()
        .find(|c| c["id"] == id)
        .expect("updated commission in list");
    assert_eq!(updated["nom"], "Sécurité renommée");
    assert_eq!(updated["besoin_chiffre"], false);
}

#[tokio::test]
async fn disabling_a_des_postes_with_existing_postes_is_rejected() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let mut body = commission_body("Logistique");
    body["a_des_postes"] = json!(true);
    let response = request(
        router.clone(),
        "POST",
        "/api/commissions",
        &token,
        Some(body),
    )
    .await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = json["id"].as_str().unwrap().to_string();

    let response = request(
        router.clone(),
        "POST",
        &format!("/api/commissions/{id}/postes"),
        &token,
        Some(json!({"nom": "Chargement"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let mut update_body = commission_body("Logistique");
    update_body["a_des_postes"] = json!(false);
    let response = request(
        router,
        "PATCH",
        &format!("/api/commissions/{id}"),
        &token,
        Some(update_body),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn adds_and_removes_a_poste() {
    let pool = spawn_test_db().await;
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let mut body = commission_body("Logistique");
    body["a_des_postes"] = json!(true);
    let response = request(
        router.clone(),
        "POST",
        "/api/commissions",
        &token,
        Some(body),
    )
    .await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let commission_id = json["id"].as_str().unwrap().to_string();

    let response = request(
        router.clone(),
        "POST",
        &format!("/api/commissions/{commission_id}/postes"),
        &token,
        Some(json!({"nom": "Chargement"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let poste_id = json["id"].as_str().unwrap().to_string();

    let response = request(router.clone(), "GET", "/api/commissions", &token, None).await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let commissions = json.as_array().unwrap();
    let commission = commissions
        .iter()
        .find(|c| c["id"] == commission_id)
        .unwrap();
    assert!(commission["postes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == poste_id));

    let response = request(
        router.clone(),
        "DELETE",
        &format!("/api/commissions/{commission_id}/postes/{poste_id}"),
        &token,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = request(router, "GET", "/api/commissions", &token, None).await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let commissions = json.as_array().unwrap();
    let commission = commissions
        .iter()
        .find(|c| c["id"] == commission_id)
        .unwrap();
    assert!(commission["postes"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn removing_a_poste_in_use_is_rejected() {
    let pool = spawn_test_db().await;
    let direct_pool = pool.clone();
    let token = insert_session(&pool).await;
    let router = app(base_state(pool), None);

    let mut body = commission_body("Logistique");
    body["a_des_postes"] = json!(true);
    let response = request(
        router.clone(),
        "POST",
        "/api/commissions",
        &token,
        Some(body),
    )
    .await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let commission_id: Uuid = json["id"].as_str().unwrap().parse().unwrap();

    let response = request(
        router.clone(),
        "POST",
        &format!("/api/commissions/{commission_id}/postes"),
        &token,
        Some(json!({"nom": "Chargement"})),
    )
    .await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let poste_id: Uuid = json["id"].as_str().unwrap().parse().unwrap();

    sqlx::query(
        "INSERT INTO mapping_souhait_form (libelle, kind, commission_id, poste_id) \
         VALUES ('Un libellé Chargement', 'commission', $1, $2)",
    )
    .bind(commission_id)
    .bind(poste_id)
    .execute(&direct_pool)
    .await
    .unwrap();

    let response = request(
        router,
        "DELETE",
        &format!("/api/commissions/{commission_id}/postes/{poste_id}"),
        &token,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
