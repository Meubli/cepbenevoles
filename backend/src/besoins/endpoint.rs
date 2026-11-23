use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthAdmin;
use crate::AppState;

use super::{
    upsert_besoin_commission, upsert_besoin_poste, BesoinCommissionError, BesoinCommissionInput,
    BesoinPosteError, BesoinPosteInput,
};

#[derive(Serialize, sqlx::FromRow)]
struct BesoinCommissionRow {
    creneau_applicatif_id: Uuid,
    commission_id: Uuid,
    nb_requis: i32,
}

#[derive(Serialize, sqlx::FromRow)]
struct BesoinPosteRow {
    creneau_applicatif_id: Uuid,
    poste_id: Uuid,
    nb_requis: i32,
}

#[derive(Serialize)]
pub struct BesoinsScreenDto {
    commission: Vec<BesoinCommissionRow>,
    poste: Vec<BesoinPosteRow>,
}

pub async fn get_besoins(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(evenement_id): Path<Uuid>,
) -> Result<Json<BesoinsScreenDto>, StatusCode> {
    let commission: Vec<BesoinCommissionRow> = sqlx::query_as(
        "SELECT creneau_applicatif_id, commission_id, nb_requis \
         FROM besoin_commission WHERE evenement_id = $1",
    )
    .bind(evenement_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let poste: Vec<BesoinPosteRow> = sqlx::query_as(
        "SELECT creneau_applicatif_id, poste_id, nb_requis \
         FROM besoin_poste WHERE evenement_id = $1",
    )
    .bind(evenement_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(BesoinsScreenDto { commission, poste }))
}

#[derive(Deserialize)]
pub struct BesoinCommissionBody {
    pub creneau_applicatif_id: Uuid,
    pub commission_id: Uuid,
    pub nb_requis: i32,
}

impl From<BesoinCommissionBody> for BesoinCommissionInput {
    fn from(b: BesoinCommissionBody) -> Self {
        BesoinCommissionInput {
            creneau_applicatif_id: b.creneau_applicatif_id,
            commission_id: b.commission_id,
            nb_requis: b.nb_requis,
        }
    }
}

fn besoin_commission_error_response(err: BesoinCommissionError) -> Response {
    match err {
        BesoinCommissionError::NbRequisNegatif
        | BesoinCommissionError::CommissionSansBesoinChiffre
        | BesoinCommissionError::CreneauHorsPerimetre => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        BesoinCommissionError::CommissionNotFound | BesoinCommissionError::CreneauNotFound => {
            StatusCode::NOT_FOUND.into_response()
        }
        BesoinCommissionError::Database(e) => {
            tracing::error!(error = %e, "besoin commission save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn patch_besoin_commission(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<BesoinCommissionBody>,
) -> Response {
    match upsert_besoin_commission(&state.db, admin_id, evenement_id, body.into()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => besoin_commission_error_response(err),
    }
}

#[derive(Deserialize)]
pub struct BesoinPosteBody {
    pub creneau_applicatif_id: Uuid,
    pub poste_id: Uuid,
    pub nb_requis: i32,
}

impl From<BesoinPosteBody> for BesoinPosteInput {
    fn from(b: BesoinPosteBody) -> Self {
        BesoinPosteInput {
            creneau_applicatif_id: b.creneau_applicatif_id,
            poste_id: b.poste_id,
            nb_requis: b.nb_requis,
        }
    }
}

fn besoin_poste_error_response(err: BesoinPosteError) -> Response {
    match err {
        BesoinPosteError::NbRequisNegatif
        | BesoinPosteError::CreneauHorsPerimetre
        | BesoinPosteError::DepasseBesoinCommission(_, _) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        BesoinPosteError::PosteNotFound | BesoinPosteError::CreneauNotFound => {
            StatusCode::NOT_FOUND.into_response()
        }
        BesoinPosteError::Database(e) => {
            tracing::error!(error = %e, "besoin poste save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn patch_besoin_poste(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<BesoinPosteBody>,
) -> Response {
    match upsert_besoin_poste(&state.db, admin_id, evenement_id, body.into()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => besoin_poste_error_response(err),
    }
}
