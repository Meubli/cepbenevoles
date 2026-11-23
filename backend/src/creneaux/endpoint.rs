use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthAdmin;
use crate::AppState;

use super::{
    create_creneau, delete_creneau, dupliquer_grille, replace_mapping, update_bornes,
    update_creneau, BornesError, BornesInput, CreneauError, CreneauInput, DupliquerError,
    MappingEntryInput, MappingError,
};

#[derive(Serialize, sqlx::FromRow)]
struct CreneauRow {
    id: Uuid,
    commission_id: Option<Uuid>,
    jour: NaiveDate,
    heure_debut: NaiveTime,
    heure_fin: NaiveTime,
    ordre: i32,
}

#[derive(Serialize, sqlx::FromRow)]
struct MappingRow {
    jour: NaiveDate,
    slot_form: String,
    creneau_applicatif_id: Uuid,
}

#[derive(Serialize, sqlx::FromRow)]
struct BornesRow {
    born_fin_matin: NaiveTime,
    born_fin_midi: NaiveTime,
    born_fin_aprem: NaiveTime,
}

#[derive(Serialize)]
pub struct CreneauxScreenDto {
    bornes: BornesRow,
    creneaux: Vec<CreneauRow>,
    mapping: Vec<MappingRow>,
}

pub async fn get_creneaux(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(evenement_id): Path<Uuid>,
) -> Result<Json<CreneauxScreenDto>, StatusCode> {
    let bornes: Option<BornesRow> = sqlx::query_as(
        "SELECT born_fin_matin, born_fin_midi, born_fin_aprem FROM evenement WHERE id = $1",
    )
    .bind(evenement_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(bornes) = bornes else {
        return Err(StatusCode::NOT_FOUND);
    };

    let creneaux: Vec<CreneauRow> = sqlx::query_as(
        "SELECT id, commission_id, jour, heure_debut, heure_fin, ordre \
         FROM creneau_applicatif WHERE evenement_id = $1 \
         ORDER BY commission_id NULLS FIRST, jour, ordre, heure_debut",
    )
    .bind(evenement_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mapping: Vec<MappingRow> = sqlx::query_as(
        "SELECT jour, slot_form, creneau_applicatif_id FROM creneau_form_mapping \
         WHERE evenement_id = $1 ORDER BY jour, slot_form",
    )
    .bind(evenement_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CreneauxScreenDto {
        bornes,
        creneaux,
        mapping,
    }))
}

#[derive(Deserialize)]
pub struct CreneauBody {
    pub commission_id: Option<Uuid>,
    pub jour: NaiveDate,
    pub heure_debut: NaiveTime,
    pub heure_fin: NaiveTime,
    #[serde(default)]
    pub ordre: i32,
}

impl From<CreneauBody> for CreneauInput {
    fn from(b: CreneauBody) -> Self {
        CreneauInput {
            commission_id: b.commission_id,
            jour: b.jour,
            heure_debut: b.heure_debut,
            heure_fin: b.heure_fin,
            ordre: b.ordre,
        }
    }
}

fn creneau_error_response(err: CreneauError) -> Response {
    match err {
        CreneauError::InvalidHeures
        | CreneauError::JourHorsEvenement
        | CreneauError::CommissionNotPropre => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        CreneauError::EvenementNotFound
        | CreneauError::CommissionNotFound
        | CreneauError::NotFound => StatusCode::NOT_FOUND.into_response(),
        CreneauError::Database(e) => {
            tracing::error!(error = %e, "creneau save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_creneau(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<CreneauBody>,
) -> Response {
    match create_creneau(&state.db, admin_id, evenement_id, body.into()).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(err) => creneau_error_response(err),
    }
}

pub async fn patch_creneau(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path((evenement_id, creneau_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CreneauBody>,
) -> Response {
    match update_creneau(&state.db, admin_id, evenement_id, creneau_id, body.into()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => creneau_error_response(err),
    }
}

pub async fn delete_creneau_endpoint(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path((evenement_id, creneau_id)): Path<(Uuid, Uuid)>,
) -> Response {
    match delete_creneau(&state.db, evenement_id, creneau_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => creneau_error_response(err),
    }
}

#[derive(Deserialize)]
pub struct BornesBody {
    pub fin_matin: NaiveTime,
    pub fin_midi: NaiveTime,
    pub fin_aprem: NaiveTime,
}

impl From<BornesBody> for BornesInput {
    fn from(b: BornesBody) -> Self {
        BornesInput {
            fin_matin: b.fin_matin,
            fin_midi: b.fin_midi,
            fin_aprem: b.fin_aprem,
        }
    }
}

pub async fn patch_bornes(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<BornesBody>,
) -> Response {
    match update_bornes(&state.db, admin_id, evenement_id, body.into()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(BornesError::NotIncreasing) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": BornesError::NotIncreasing.to_string()})),
        )
            .into_response(),
        Err(BornesError::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(err @ BornesError::Database(_)) => {
            tracing::error!(error = %err, "bornes save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct DupliquerBody {
    pub source_commission_id: Uuid,
    pub target_commission_id: Uuid,
}

pub async fn post_dupliquer(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<DupliquerBody>,
) -> Response {
    match dupliquer_grille(
        &state.db,
        admin_id,
        evenement_id,
        body.source_commission_id,
        body.target_commission_id,
    )
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(
            err @ (DupliquerError::SourceEmpty
            | DupliquerError::TargetNotEmpty
            | DupliquerError::TargetNotPropre),
        ) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        Err(DupliquerError::SourceNotFound | DupliquerError::TargetNotFound) => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(err @ DupliquerError::Database(_)) => {
            tracing::error!(error = %err, "grille duplication failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct MappingEntryBody {
    pub jour: NaiveDate,
    pub slot_form: String,
    pub creneau_applicatif_id: Uuid,
}

impl From<MappingEntryBody> for MappingEntryInput {
    fn from(b: MappingEntryBody) -> Self {
        MappingEntryInput {
            jour: b.jour,
            slot_form: b.slot_form,
            creneau_applicatif_id: b.creneau_applicatif_id,
        }
    }
}

pub async fn patch_mapping(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<Vec<MappingEntryBody>>,
) -> Response {
    let entries = body.into_iter().map(Into::into).collect();
    match replace_mapping(&state.db, evenement_id, entries).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err @ (MappingError::InvalidSlot | MappingError::InvalidCreneau(_))) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        Err(err @ MappingError::Database(_)) => {
            tracing::error!(error = %err, "mapping save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
