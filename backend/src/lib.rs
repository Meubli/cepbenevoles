pub mod auth;
pub mod config;
pub mod correlation;
pub mod google_auth;
pub mod health;
pub mod mapping;
pub mod sheets;
pub mod sync;

use std::sync::Arc;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;
use tokio::sync::Mutex;
use tower_cookies::CookieManagerLayer;
use tower_http::services::{ServeDir, ServeFile};

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

/// Every backend route lives under `/api` (except `/health`, an infra
/// probe path, not a page) so a SPA page and an API route never collide —
/// e.g. the future `/mapping` page vs an API route of the same name.
fn api_router() -> Router<AppState> {
    Router::new()
        .route("/auth/request-code", post(auth::request_code))
        .route("/auth/verify-code", post(auth::verify_code))
        .route("/session", get(auth::require_session))
        .route("/sync", post(sync::endpoint::trigger_sync))
        .route(
            "/mapping",
            get(mapping::endpoint::list_mapping).post(mapping::endpoint::post_mapping),
        )
        .route("/commissions", get(mapping::endpoint::list_commissions))
        // Without this, an unmatched /api/* path falls through to the SPA
        // fallback below and gets served index.html as if it were a page —
        // api.ts would then try to JSON-parse an HTML body.
        .fallback(|| async { axum::http::StatusCode::NOT_FOUND })
}

/// `static_dir` is the built SvelteKit SPA (technical-specs.md §3); absent
/// in local dev, where `vite dev` serves the frontend separately and
/// proxies `/api` to this backend.
pub fn app(state: AppState, static_dir: Option<&str>) -> Router {
    let mut router = Router::new()
        .route("/health", get(health::health))
        .nest("/api", api_router())
        .layer(CookieManagerLayer::new())
        .layer(middleware::from_fn(correlation::correlation_id))
        .with_state(state);

    if let Some(dir) = static_dir {
        let index = format!("{dir}/index.html");
        // `.fallback()`, not `.not_found_service()` — the latter forces the
        // response status to 404 regardless of what the fallback service
        // itself returns (tower_http::services::fs::serve_dir::SetStatus),
        // which would make every SPA deep link an "index.html with a 404".
        let serve_dir = ServeDir::new(dir).fallback(ServeFile::new(index));
        router = router.fallback_service(serve_dir);
    }

    router
}
