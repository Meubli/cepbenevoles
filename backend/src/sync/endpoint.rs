//! Manual "Synchroniser maintenant" trigger + status (specs.md §3.3, §7.3).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use tower_cookies::Cookies;

use crate::auth::{session_valid, AuthAdmin};
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

#[derive(Serialize, sqlx::FromRow)]
pub struct SheetStatus {
    pub sheet_id: String,
    /// Names of the non-archived events sharing this sheet (specs.md §3.3).
    pub event_names: String,
    pub soumission_count: i64,
    pub last_started_at: Option<DateTime<Utc>>,
    pub last_finished_at: Option<DateTime<Utc>>,
    pub last_inserted: Option<i32>,
    pub last_already_present: Option<i32>,
    pub last_unrouted: Option<i32>,
    /// Set when the whole last run failed for this sheet, rather than a
    /// per-row problem (cf. `sync_run.error`).
    pub last_error: Option<String>,
}

/// One row per distinct `sheet_id` used by a non-archived event, with its
/// submission count and most recent `sync_run` (specs.md §7.3's "suivi de
/// synchronisation"). A sheet that has never been synced yet still shows up
/// (event just got a `sheet_id`), with every `last_*` field null.
pub async fn get_sync_status(
    State(state): State<AppState>,
    _admin: AuthAdmin,
) -> Result<Json<Vec<SheetStatus>>, StatusCode> {
    let rows: Vec<SheetStatus> = sqlx::query_as(
        "WITH sheets AS ( \
             SELECT sheet_id, string_agg(nom, ', ' ORDER BY nom) AS event_names \
             FROM evenement \
             WHERE archive = false AND sheet_id IS NOT NULL \
             GROUP BY sheet_id \
         ), \
         counts AS ( \
             SELECT e.sheet_id, COUNT(s.id) AS soumission_count \
             FROM evenement e \
             LEFT JOIN soumission s ON s.evenement_id = e.id \
             WHERE e.sheet_id IS NOT NULL \
             GROUP BY e.sheet_id \
         ), \
         latest_run AS ( \
             SELECT DISTINCT ON (sheet_id) \
                 sheet_id, started_at, finished_at, inserted, already_present, unrouted, error \
             FROM sync_run \
             ORDER BY sheet_id, started_at DESC \
         ) \
         SELECT \
             sh.sheet_id, \
             sh.event_names, \
             COALESCE(c.soumission_count, 0) AS soumission_count, \
             lr.started_at AS last_started_at, \
             lr.finished_at AS last_finished_at, \
             lr.inserted AS last_inserted, \
             lr.already_present AS last_already_present, \
             lr.unrouted AS last_unrouted, \
             lr.error AS last_error \
         FROM sheets sh \
         LEFT JOIN counts c ON c.sheet_id = sh.sheet_id \
         LEFT JOIN latest_run lr ON lr.sheet_id = sh.sheet_id \
         ORDER BY sh.sheet_id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "get_sync_status query failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}
