use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthAdmin;
use crate::AppState;

use super::{add_poste, create_commission, remove_poste, update_commission};
use super::{CommissionInput, PosteError, SaveCommissionError};

#[derive(Serialize, sqlx::FromRow)]
struct CommissionRow {
    id: Uuid,
    nom: String,
    prerequis_libre_texte: Option<String>,
    a_des_postes: bool,
    grille_creneaux: String,
    besoin_chiffre: bool,
    couleur: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct PosteRow {
    id: Uuid,
    commission_id: Uuid,
    nom: String,
    prerequis_libre_texte: Option<String>,
}

#[derive(Serialize)]
pub struct CommissionDto {
    id: Uuid,
    nom: String,
    prerequis_libre_texte: Option<String>,
    a_des_postes: bool,
    grille_creneaux: String,
    besoin_chiffre: bool,
    couleur: String,
    postes: Vec<PosteDto>,
}

#[derive(Serialize)]
struct PosteDto {
    id: Uuid,
    nom: String,
    prerequis_libre_texte: Option<String>,
}

/// Referential used by the Commissions screen itself and by the Mapping
/// screen's "Commission" / "Poste" picker. No `actif` filter toggle exists
/// yet (scope chosen with the user) — only active commissions are ever
/// returned.
pub async fn list_commissions(
    State(state): State<AppState>,
    _admin: AuthAdmin,
) -> Result<Json<Vec<CommissionDto>>, StatusCode> {
    let commissions: Vec<CommissionRow> = sqlx::query_as(
        "SELECT id, nom, prerequis_libre_texte, a_des_postes, grille_creneaux, besoin_chiffre, couleur \
         FROM commission WHERE actif = true ORDER BY nom",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let postes: Vec<PosteRow> = sqlx::query_as(
        "SELECT id, commission_id, nom, prerequis_libre_texte FROM poste ORDER BY nom",
    )
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
                prerequis_libre_texte: p.prerequis_libre_texte,
            });
    }

    let dtos = commissions
        .into_iter()
        .map(|c| CommissionDto {
            postes: postes_by_commission.remove(&c.id).unwrap_or_default(),
            id: c.id,
            nom: c.nom,
            prerequis_libre_texte: c.prerequis_libre_texte,
            a_des_postes: c.a_des_postes,
            grille_creneaux: c.grille_creneaux,
            besoin_chiffre: c.besoin_chiffre,
            couleur: c.couleur,
        })
        .collect();

    Ok(Json(dtos))
}

#[derive(Deserialize)]
pub struct CommissionBody {
    pub nom: String,
    pub prerequis_libre_texte: Option<String>,
    pub a_des_postes: bool,
    pub grille_creneaux: String,
    pub besoin_chiffre: bool,
    pub couleur: String,
}

impl From<CommissionBody> for CommissionInput {
    fn from(b: CommissionBody) -> Self {
        CommissionInput {
            nom: b.nom,
            prerequis_libre_texte: b.prerequis_libre_texte,
            a_des_postes: b.a_des_postes,
            grille_creneaux: b.grille_creneaux,
            besoin_chiffre: b.besoin_chiffre,
            couleur: b.couleur,
        }
    }
}

fn commission_error_response(err: SaveCommissionError) -> Response {
    match err {
        SaveCommissionError::EmptyNom
        | SaveCommissionError::NomTooLong
        | SaveCommissionError::PrerequisTooLong
        | SaveCommissionError::DuplicateNom(_)
        | SaveCommissionError::InvalidGrille
        | SaveCommissionError::InvalidCouleur
        | SaveCommissionError::PostesRemain => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        SaveCommissionError::NotFound => StatusCode::NOT_FOUND.into_response(),
        SaveCommissionError::Database(e) => {
            tracing::error!(error = %e, "commission save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_commission(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Json(body): Json<CommissionBody>,
) -> Response {
    match create_commission(&state.db, admin_id, body.into()).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(err) => commission_error_response(err),
    }
}

pub async fn patch_commission(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<CommissionBody>,
) -> Response {
    match update_commission(&state.db, admin_id, id, body.into()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => commission_error_response(err),
    }
}

#[derive(Deserialize)]
pub struct PosteBody {
    pub nom: String,
}

fn poste_error_response(err: PosteError) -> Response {
    match err {
        PosteError::EmptyNom
        | PosteError::NomTooLong
        | PosteError::DuplicateNom(_)
        | PosteError::InUse => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        PosteError::CommissionNotFound | PosteError::NotFound => {
            StatusCode::NOT_FOUND.into_response()
        }
        PosteError::Database(e) => {
            tracing::error!(error = %e, "poste management failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_poste(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(commission_id): Path<Uuid>,
    Json(body): Json<PosteBody>,
) -> Response {
    match add_poste(&state.db, admin_id, commission_id, body.nom).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(err) => poste_error_response(err),
    }
}

pub async fn delete_poste(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path((commission_id, poste_id)): Path<(Uuid, Uuid)>,
) -> Response {
    match remove_poste(&state.db, commission_id, poste_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => poste_error_response(err),
    }
}
