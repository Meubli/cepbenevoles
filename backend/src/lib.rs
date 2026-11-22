pub mod auth;
pub mod config;
pub mod correlation;
pub mod google_auth;
pub mod health;
pub mod sheets;
pub mod sync;

use std::sync::Arc;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;
use tokio::sync::Mutex;
use tower_cookies::CookieManagerLayer;

use google_auth::GoogleAuth;
use sheets::SheetsClient;

/// Present only when a service-account key is configured — local dev and
/// the current e2e setup (no Google credentials) still boot without it,
/// the manual sync endpoint just answers 503 and the scheduler doesn't run.
#[derive(Clone)]
pub struct SyncContext {
    pub auth: Arc<GoogleAuth>,
    pub sheets: SheetsClient,
    /// Held for the duration of a sync run so a second trigger (manual or
    /// scheduled) gets a 409 instead of racing a duplicate fetch — `ON
    /// CONFLICT DO NOTHING` already makes concurrent persistence safe, this
    /// is just to avoid two redundant Sheets fetches and confusing counts.
    pub running: Arc<Mutex<()>>,
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub smtp_from: String,
    pub sync: Option<SyncContext>,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/auth/request-code", post(auth::request_code))
        .route("/auth/verify-code", post(auth::verify_code))
        .route("/admin", get(auth::require_session))
        .route("/admin/sync", post(sync::endpoint::trigger_sync))
        .layer(CookieManagerLayer::new())
        .layer(middleware::from_fn(correlation::correlation_id))
        .with_state(state)
}
