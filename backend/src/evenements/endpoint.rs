use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthAdmin;
use crate::AppState;

use super::{
    create_evenement, set_archive, update_evenement, EvenementInput, SaveEvenementError,
    SetArchiveError, SheetIdError,
};

#[derive(Serialize, sqlx::FromRow)]
pub struct EvenementDto {
    id: Uuid,
    nom: String,
    date_debut: NaiveDate,
    date_fin: NaiveDate,
    lieu: Option<String>,
    archive: bool,
    sheet_id: Option<String>,
    bloc_formulaire: Option<String>,
}

pub async fn list_evenements(
    State(state): State<AppState>,
    _admin: AuthAdmin,
) -> Result<Json<Vec<EvenementDto>>, StatusCode> {
    let rows: Vec<EvenementDto> = sqlx::query_as(
        "SELECT id, nom, date_debut, date_fin, lieu, archive, sheet_id, bloc_formulaire \
         FROM evenement ORDER BY date_debut",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct EvenementBody {
    pub nom: String,
    pub date_debut: NaiveDate,
    pub date_fin: NaiveDate,
    pub lieu: Option<String>,
    pub sheet_id: Option<String>,
    pub bloc_formulaire: Option<String>,
}

impl From<EvenementBody> for EvenementInput {
    fn from(b: EvenementBody) -> Self {
        EvenementInput {
            nom: b.nom,
            date_debut: b.date_debut,
            date_fin: b.date_fin,
            lieu: b.lieu,
            sheet_id: b.sheet_id,
            bloc_formulaire: b.bloc_formulaire,
        }
    }
}

fn unprocessable(err: &impl ToString) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"error": err.to_string()})),
    )
        .into_response()
}

fn save_error_response(err: SaveEvenementError) -> Response {
    match err {
        SaveEvenementError::EmptyNom
        | SaveEvenementError::InvalidDates
        | SaveEvenementError::SheetId(SheetIdError::NotFound)
        | SaveEvenementError::SheetId(SheetIdError::Forbidden)
        | SaveEvenementError::SheetId(SheetIdError::InvalidFormat)
        | SaveEvenementError::Bloc(_) => unprocessable(&err),
        SaveEvenementError::NotFound => StatusCode::NOT_FOUND.into_response(),
        SaveEvenementError::SheetId(other) => {
            tracing::error!(error = %other, "sheet_id validation failed unexpectedly");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        SaveEvenementError::Database(e) => {
            tracing::error!(error = %e, "evenement save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_evenement(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Json(body): Json<EvenementBody>,
) -> Response {
    match create_evenement(&state.db, state.sync.as_ref(), admin_id, body.into()).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(err) => save_error_response(err),
    }
}

pub async fn patch_evenement(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<EvenementBody>,
) -> Response {
    match update_evenement(&state.db, state.sync.as_ref(), admin_id, id, body.into()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => save_error_response(err),
    }
}

#[derive(Deserialize)]
pub struct ArchiveBody {
    pub archive: bool,
}

pub async fn patch_archive(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<ArchiveBody>,
) -> Response {
    match set_archive(&state.db, admin_id, id, body.archive).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(SetArchiveError::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(err @ SetArchiveError::Bloc(_)) => unprocessable(&err),
        Err(err @ SetArchiveError::Database(_)) => {
            tracing::error!(error = %err, "archive toggle failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Blocs the current form knows how to read, for the Événements screen's
/// bloc picker (specs.md §3.3) — the same list `bloc_to_slug` validates
/// against, so the UI can never offer a choice the backend would reject.
pub async fn list_blocs_formulaire(_admin: AuthAdmin) -> Json<Vec<serde_json::Value>> {
    let blocs = crate::sync::form_2027::known_blocs()
        .into_iter()
        .map(|(value, label)| json!({"value": value, "label": label}))
        .collect();
    Json(blocs)
}
