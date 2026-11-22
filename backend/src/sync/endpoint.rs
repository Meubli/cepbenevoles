//! Manual "Synchroniser maintenant" trigger (specs.md §3.3, §7.3).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use tower_cookies::Cookies;

use crate::auth::session_valid;
use crate::AppState;

use super::orchestrate::sync_all_and_record;

pub async fn trigger_sync(State(state): State<AppState>, cookies: Cookies) -> Response {
    if !session_valid(&state, &cookies).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let Some(sync_ctx) = &state.sync else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "synchro non configurée (pas de clé de service account)"})),
        )
            .into_response();
    };

    let Ok(_guard) = sync_ctx.running.try_lock() else {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "synchro déjà en cours"})),
        )
            .into_response();
    };

    match sync_all_and_record(&state.db, &sync_ctx.auth, &sync_ctx.sheets).await {
        Ok(outcomes) => {
            let summary: Vec<_> = outcomes
                .iter()
                .map(|o| match &o.result {
                    Ok((report, row_errors)) => json!({
                        "sheet_id": o.sheet_id,
                        "status": "ok",
                        "inserted": report.inserted,
                        "already_present": report.already_present,
                        "unrouted": report.unrouted,
                        "row_errors": row_errors.len(),
                    }),
                    Err(err) => json!({
                        "sheet_id": o.sheet_id,
                        "status": "error",
                        "error": err.to_string(),
                    }),
                })
                .collect();
            Json(json!({ "sheets": summary })).into_response()
        }
        // A database error here is the one genuinely unexpected failure
        // mode (Google-side errors are already captured per-sheet above) —
        // let it become a 5xx, which the correlation-id middleware turns
        // into a body carrying an id an admin can grep the logs for.
        Err(err) => {
            tracing::error!(error = %err, "sync_all_and_record failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
