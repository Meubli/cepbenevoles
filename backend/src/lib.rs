pub mod auth;
pub mod config;
pub mod health;
pub mod sheets;
pub mod sync;

use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;
use tower_cookies::CookieManagerLayer;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub smtp_from: String,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/auth/request-code", post(auth::request_code))
        .route("/auth/verify-code", post(auth::verify_code))
        .route("/admin", get(auth::require_session))
        .layer(CookieManagerLayer::new())
        .with_state(state)
}
