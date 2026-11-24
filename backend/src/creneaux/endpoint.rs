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
    create_creneau, create_creneau_repas, delete_creneau, delete_creneau_repas, dupliquer_grille,
    dupliquer_jour, replace_mapping, update_creneau, update_creneau_repas, CreneauError,
    CreneauInput, CreneauRepasError, CreneauRepasInput, DupliquerError, DupliquerJourError,
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
struct CreneauRepasRow {
    id: Uuid,
    jour: NaiveDate,
    nom: String,
}

#[derive(Serialize)]
struct CreneauRepasDto {
    id: Uuid,
    jour: NaiveDate,
    nom: String,
    creneau_ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct CreneauxScreenDto {
    creneaux: Vec<CreneauRow>,
    mapping: Vec<MappingRow>,
    creneaux_repas: Vec<CreneauRepasDto>,
}

pub async fn get_creneaux(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(evenement_id): Path<Uuid>,
) -> Result<Json<CreneauxScreenDto>, StatusCode> {
    let evenement_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM evenement WHERE id = $1)")
            .bind(evenement_id)
            .fetch_one(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !evenement_exists {
        return Err(StatusCode::NOT_FOUND);
    }

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

    let repas_rows: Vec<CreneauRepasRow> = sqlx::query_as(
        "SELECT id, jour, nom FROM creneau_repas WHERE evenement_id = $1 ORDER BY jour, nom",
    )
    .bind(evenement_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut creneaux_repas = Vec::with_capacity(repas_rows.len());
    for r in repas_rows {
        let creneau_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT creneau_applicatif_id FROM creneau_repas_membre WHERE creneau_repas_id = $1",
        )
        .bind(r.id)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        creneaux_repas.push(CreneauRepasDto {
            id: r.id,
            jour: r.jour,
            nom: r.nom,
            creneau_ids,
        });
    }

    Ok(Json(CreneauxScreenDto {
        creneaux,
        mapping,
        creneaux_repas,
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
pub struct DupliquerJourBody {
    pub source_jour: NaiveDate,
    pub target_jour: NaiveDate,
}

pub async fn post_dupliquer_jour(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<DupliquerJourBody>,
) -> Response {
    match dupliquer_jour(
        &state.db,
        admin_id,
        evenement_id,
        body.source_jour,
        body.target_jour,
    )
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(
            err @ (DupliquerJourError::SameJour
            | DupliquerJourError::JourHorsEvenement
            | DupliquerJourError::SourceEmpty
            | DupliquerJourError::TargetNotEmpty),
        ) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        Err(DupliquerJourError::EvenementNotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(err @ DupliquerJourError::Database(_)) => {
            tracing::error!(error = %err, "jour duplication failed");
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

#[derive(Deserialize)]
pub struct CreneauRepasBody {
    pub jour: NaiveDate,
    pub nom: String,
    pub creneau_ids: Vec<Uuid>,
}

impl From<CreneauRepasBody> for CreneauRepasInput {
    fn from(b: CreneauRepasBody) -> Self {
        CreneauRepasInput {
            jour: b.jour,
            nom: b.nom,
            creneau_ids: b.creneau_ids,
        }
    }
}

fn creneau_repas_error_response(err: CreneauRepasError) -> Response {
    match err {
        CreneauRepasError::EmptyNom
        | CreneauRepasError::NotEnoughMembres
        | CreneauRepasError::JourHorsEvenement
        | CreneauRepasError::InvalidCreneau => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        CreneauRepasError::EvenementNotFound | CreneauRepasError::NotFound => {
            StatusCode::NOT_FOUND.into_response()
        }
        CreneauRepasError::Database(e) => {
            tracing::error!(error = %e, "creneau repas save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_creneau_repas(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<CreneauRepasBody>,
) -> Response {
    match create_creneau_repas(&state.db, admin_id, evenement_id, body.into()).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(err) => creneau_repas_error_response(err),
    }
}

pub async fn patch_creneau_repas(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path((evenement_id, creneau_repas_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CreneauRepasBody>,
) -> Response {
    match update_creneau_repas(
        &state.db,
        admin_id,
        evenement_id,
        creneau_repas_id,
        body.into(),
    )
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => creneau_repas_error_response(err),
    }
}

pub async fn delete_creneau_repas_endpoint(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path((evenement_id, creneau_repas_id)): Path<(Uuid, Uuid)>,
) -> Response {
    match delete_creneau_repas(&state.db, evenement_id, creneau_repas_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => creneau_repas_error_response(err),
    }
}
