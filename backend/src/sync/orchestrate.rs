//! Wires together auth, fetch, parse and persist for every event that has a
//! `sheet_id` configured — one sheet fetched once even when several events
//! share it (specs.md §3.3). Scheduling (periodic job, manual trigger) lives
//! in the caller; this module is the orchestration logic itself.

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use crate::google_auth::{AuthError, GoogleAuth};
use crate::sheets::{SheetsClient, SheetsError};

use super::form_2027::{self, TAB_NAME};
use super::persist::{persist_parsed_rows, SyncReport};
use super::{EventSlug, RowError};

#[derive(Debug, thiserror::Error)]
pub enum SheetSyncError {
    #[error("evenement {evenement_id} has sheet_id set but no bloc_formulaire configured")]
    MissingBloc { evenement_id: Uuid },
    #[error("evenement {evenement_id} has an unrecognized bloc_formulaire \"{bloc}\"")]
    UnknownBloc { evenement_id: Uuid, bloc: String },
    #[error("blocs \"{bloc}\" claimed by both {first} and {second} on the same sheet")]
    DuplicateBloc {
        bloc: String,
        first: Uuid,
        second: Uuid,
    },
    #[error("sheet header is missing required column(s): {0:?}")]
    InvalidHeader(Vec<&'static str>),
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    #[error("sheets error: {0}")]
    Sheets(#[from] SheetsError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct SheetSyncOutcome {
    pub sheet_id: String,
    pub result: Result<(SyncReport, Vec<RowError>), SheetSyncError>,
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    sheet_id: Option<String>,
    bloc_formulaire: Option<String>,
}

/// Syncs every non-archived event with a `sheet_id` set. A failure on one
/// sheet (bad config, Google error, …) never blocks the others — each sheet
/// gets its own outcome.
pub async fn sync_all(
    pool: &PgPool,
    auth: &GoogleAuth,
    sheets: &SheetsClient,
) -> Result<Vec<SheetSyncOutcome>, sqlx::Error> {
    let events: Vec<EventRow> = sqlx::query_as(
        "SELECT id, sheet_id, bloc_formulaire FROM evenement \
         WHERE archive = false AND sheet_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    let mut by_sheet: HashMap<String, Vec<EventRow>> = HashMap::new();
    for event in events {
        let sheet_id = event
            .sheet_id
            .clone()
            .expect("filtered to sheet_id IS NOT NULL above");
        by_sheet.entry(sheet_id).or_default().push(event);
    }

    let mut outcomes = Vec::with_capacity(by_sheet.len());
    for (sheet_id, events) in by_sheet {
        let result = sync_one_sheet(pool, auth, sheets, &sheet_id, &events).await;
        outcomes.push(SheetSyncOutcome { sheet_id, result });
    }
    Ok(outcomes)
}

async fn sync_one_sheet(
    pool: &PgPool,
    auth: &GoogleAuth,
    sheets: &SheetsClient,
    sheet_id: &str,
    events: &[EventRow],
) -> Result<(SyncReport, Vec<RowError>), SheetSyncError> {
    let routing = route_events(events)?;

    let token = auth.access_token().await?;
    let values = sheets.fetch_values(&token, sheet_id, TAB_NAME).await?;

    let Some((header, data_rows)) = values.split_first() else {
        return Ok((SyncReport::default(), Vec::new()));
    };

    form_2027::validate_header(header).map_err(SheetSyncError::InvalidHeader)?;

    let mut parsed = Vec::with_capacity(data_rows.len());
    let mut row_errors = Vec::new();
    for (i, row) in data_rows.iter().enumerate() {
        // Sheet row numbers are 1-based with the header as row 1, so the
        // first data row (data_rows[0]) is row 2 — matches what an admin
        // sees when looking at the Sheet directly.
        let sheet_row_number = i + 2;
        match form_2027::parse_row(header, row, sheet_row_number) {
            Ok(row) => parsed.push(row),
            Err(err) => row_errors.push(err),
        }
    }

    let report = persist_parsed_rows(pool, &routing, &parsed).await?;

    // A submission inserted here may already have a mapping (or may be the
    // one that unblocks a mapping race, cf. specs.md §3.2) — sweep now
    // rather than waiting for an admin to save an unrelated mapping.
    crate::mapping::resolve_pending_libelles(pool).await?;

    Ok((report, row_errors))
}

/// Runs [`sync_all`] and records one `sync_run` row per sheet (plus one
/// `sync_row_error` row per parse error), so the Synchro screen can show
/// "dernière synchro" / "+N depuis" / errors (specs.md §7.3) without
/// scraping container logs. The caller (scheduler, manual-sync endpoint)
/// only needs the returned outcomes for its own immediate response.
pub async fn sync_all_and_record(
    pool: &PgPool,
    auth: &GoogleAuth,
    sheets: &SheetsClient,
) -> Result<Vec<SheetSyncOutcome>, sqlx::Error> {
    let started_at = chrono::Utc::now();
    let outcomes = sync_all(pool, auth, sheets).await?;

    for outcome in &outcomes {
        match &outcome.result {
            Ok((report, row_errors)) => {
                let run_id: Uuid = sqlx::query_scalar(
                    "INSERT INTO sync_run \
                        (sheet_id, started_at, finished_at, inserted, already_present, unrouted, skipped_availabilities) \
                     VALUES ($1, $2, now(), $3, $4, $5, $6) \
                     RETURNING id",
                )
                .bind(&outcome.sheet_id)
                .bind(started_at)
                .bind(report.inserted as i32)
                .bind(report.already_present as i32)
                .bind(report.unrouted as i32)
                .bind(report.skipped_availabilities as i32)
                .fetch_one(pool)
                .await?;

                for err in row_errors {
                    let (sheet_row, message) = row_error_detail(err);
                    sqlx::query(
                        "INSERT INTO sync_row_error (sync_run_id, sheet_row, message) \
                         VALUES ($1, $2, $3)",
                    )
                    .bind(run_id)
                    .bind(sheet_row)
                    .bind(message)
                    .execute(pool)
                    .await?;
                }
            }
            Err(err) => {
                sqlx::query(
                    "INSERT INTO sync_run (sheet_id, started_at, finished_at, error) \
                     VALUES ($1, $2, now(), $3)",
                )
                .bind(&outcome.sheet_id)
                .bind(started_at)
                .bind(err.to_string())
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(outcomes)
}

fn row_error_detail(err: &RowError) -> (i32, String) {
    let row_index = match err {
        RowError::MissingField { row_index, .. }
        | RowError::UnrecognizedValue { row_index, .. }
        | RowError::InvalidHorodateur { row_index, .. } => *row_index,
    };
    (row_index as i32, err.to_string())
}

/// Builds the `EventSlug -> Evenement id` routing table for one sheet,
/// checking config problems before any network call: a missing, unknown,
/// or duplicated `bloc_formulaire` is a config error to report, not
/// something to silently drop or overwrite in a `HashMap::insert`.
fn route_events(events: &[EventRow]) -> Result<HashMap<EventSlug, Uuid>, SheetSyncError> {
    let mut routing = HashMap::new();
    let mut claimed_by: HashMap<String, Uuid> = HashMap::new();

    for event in events {
        let bloc = event
            .bloc_formulaire
            .clone()
            .ok_or(SheetSyncError::MissingBloc {
                evenement_id: event.id,
            })?;

        if let Some(&first) = claimed_by.get(&bloc) {
            return Err(SheetSyncError::DuplicateBloc {
                bloc,
                first,
                second: event.id,
            });
        }
        claimed_by.insert(bloc.clone(), event.id);

        let slug = form_2027::bloc_to_slug(&bloc).ok_or(SheetSyncError::UnknownBloc {
            evenement_id: event.id,
            bloc,
        })?;
        routing.insert(slug, event.id);
    }

    Ok(routing)
}
