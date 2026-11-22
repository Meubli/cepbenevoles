use std::collections::HashMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthAdmin;
use crate::AppState;

use super::{save_mapping, MappingKind, SaveMapping, SaveMappingError};

#[derive(Serialize, sqlx::FromRow)]
pub struct MappingRow {
    pub libelle: String,
    pub kind: Option<String>,
    pub commission_nom: Option<String>,
    pub poste_nom: Option<String>,
    pub count: i64,
}

/// One row per distinct raw label seen in non-archived events' submissions
/// (an unmapped label from an archived event must not block génération
/// forever — specs.md §6.1, §12.6).
pub async fn list_mapping(
    State(state): State<AppState>,
    _admin: AuthAdmin,
) -> Result<Json<Vec<MappingRow>>, StatusCode> {
    let rows: Vec<MappingRow> = sqlx::query_as(
        "SELECT sl.libelle_brut AS libelle, \
                m.kind, \
                c.nom AS commission_nom, \
                p.nom AS poste_nom, \
                COUNT(DISTINCT sl.soumission_id) AS count \
         FROM soumission_libelle sl \
         JOIN soumission s ON s.id = sl.soumission_id \
         JOIN evenement e ON e.id = s.evenement_id \
         LEFT JOIN mapping_souhait_form m ON m.libelle = sl.libelle_brut \
         LEFT JOIN commission c ON c.id = m.commission_id \
         LEFT JOIN poste p ON p.id = m.poste_id \
         WHERE e.archive = false \
         GROUP BY sl.libelle_brut, m.kind, c.nom, p.nom \
         ORDER BY sl.libelle_brut",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct SaveMappingBody {
    pub libelle: String,
    pub kind: String,
    pub commission_id: Option<Uuid>,
    pub poste_id: Option<Uuid>,
}

pub async fn post_mapping(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Json(body): Json<SaveMappingBody>,
) -> Response {
    let kind = match body.kind.as_str() {
        "commission" => MappingKind::Commission,
        "polyvalent" => MappingKind::Polyvalent,
        _ => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "kind doit être \"commission\" ou \"polyvalent\""})),
            )
                .into_response()
        }
    };

    let result = save_mapping(
        &state.db,
        admin_id,
        SaveMapping {
            libelle: body.libelle,
            kind,
            commission_id: body.commission_id,
            poste_id: body.poste_id,
        },
    )
    .await;

    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(SaveMappingError::UnknownLabel) => StatusCode::NOT_FOUND.into_response(),
        Err(
            err @ (SaveMappingError::InvalidCommission
            | SaveMappingError::PosteNotInCommission
            | SaveMappingError::CommissionHasNoPostes),
        ) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        Err(SaveMappingError::Database(err)) => {
            tracing::error!(error = %err, "save_mapping failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Serialize, sqlx::FromRow)]
struct CommissionRow {
    id: Uuid,
    nom: String,
    a_des_postes: bool,
}

#[derive(Serialize, sqlx::FromRow)]
struct PosteRow {
    id: Uuid,
    commission_id: Uuid,
    nom: String,
}

#[derive(Serialize)]
pub struct CommissionDto {
    id: Uuid,
    nom: String,
    a_des_postes: bool,
    postes: Vec<PosteDto>,
}

#[derive(Serialize)]
struct PosteDto {
    id: Uuid,
    nom: String,
}

/// Referential for the mapping screen's "Commission" / "Poste" picker.
pub async fn list_commissions(
    State(state): State<AppState>,
    _admin: AuthAdmin,
) -> Result<Json<Vec<CommissionDto>>, StatusCode> {
    let commissions: Vec<CommissionRow> = sqlx::query_as(
        "SELECT id, nom, a_des_postes FROM commission WHERE actif = true ORDER BY nom",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let postes: Vec<PosteRow> =
        sqlx::query_as("SELECT id, commission_id, nom FROM poste ORDER BY nom")
            .fetch_all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut postes_by_commission: HashMap<Uuid, Vec<PosteDto>> = HashMap::new();
    for p in postes {
        postes_by_commission
            .entry(p.commission_id)
            .or_default()
            .push(PosteDto {
                id: p.id,
                nom: p.nom,
            });
    }

    let dtos = commissions
        .into_iter()
        .map(|c| CommissionDto {
            postes: postes_by_commission.remove(&c.id).unwrap_or_default(),
            id: c.id,
            nom: c.nom,
            a_des_postes: c.a_des_postes,
        })
        .collect();

    Ok(Json(dtos))
}
