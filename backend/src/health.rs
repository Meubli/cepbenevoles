use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;

/// Proves the sqlx offline-mode plumbing (`cargo sqlx prepare` / `.sqlx/`)
/// works end to end, ahead of any real query existing yet.
pub async fn health(State(state): State<AppState>) -> StatusCode {
    match sqlx::query!("SELECT 1 AS one").fetch_one(&state.db).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
