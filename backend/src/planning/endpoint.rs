use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthAdmin;
use crate::AppState;

use super::{create_affectation, delete_affectation, AffectationError, CreateAffectationInput};

#[derive(Deserialize)]
pub struct JourQuery {
    pub jour: NaiveDate,
}

#[derive(Serialize, sqlx::FromRow)]
struct SoumissionJourRow {
    id: Uuid,
    nom: String,
    prenom: String,
    permis_b: bool,
    polyvalent: bool,
}

#[derive(Serialize, sqlx::FromRow)]
struct SouhaitDto {
    commission_id: Uuid,
    poste_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct BenevoleJourDto {
    soumission_id: Uuid,
    nom: String,
    prenom: String,
    permis_b: bool,
    polyvalent: bool,
    creneau_ids: Vec<Uuid>,
    souhaits: Vec<SouhaitDto>,
}

/// Bénévoles (soumissions) ayant déclaré une disponibilité ce jour-là sur
/// cet événement — même quand ils n'ont encore aucune affectation —, avec
/// leur disponibilité déduite du mapping form → créneaux applicatifs
/// (§4.1) côté grille générale uniquement (seule grille que le form peut
/// atteindre, cf. `creneaux::replace_mapping`), et leurs souhaits de
/// commission (§3.1), pour que l'écran Planning puisse construire ses
/// lignes et proposer l'affectation manuelle sans dépendre d'une
/// affectation déjà posée.
pub async fn list_benevoles_jour(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Query(query): Query<JourQuery>,
) -> Result<Json<Vec<BenevoleJourDto>>, StatusCode> {
    let soumissions: Vec<SoumissionJourRow> = sqlx::query_as(
        "SELECT DISTINCT s.id, s.nom, s.prenom, s.permis_b, s.polyvalent \
         FROM soumission s \
         JOIN disponibilite_soumission d ON d.soumission_id = s.id AND d.jour = $2 \
         JOIN creneau_form_mapping m \
             ON m.evenement_id = s.evenement_id AND m.jour = d.jour AND m.slot_form = d.slot \
         WHERE s.evenement_id = $1 \
         ORDER BY s.nom, s.prenom",
    )
    .bind(evenement_id)
    .bind(query.jour)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut out = Vec::with_capacity(soumissions.len());
    for s in soumissions {
        let creneau_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT m.creneau_applicatif_id \
             FROM disponibilite_soumission d \
             JOIN creneau_form_mapping m \
                 ON m.evenement_id = $1 AND m.jour = d.jour AND m.slot_form = d.slot \
             WHERE d.soumission_id = $2 AND d.jour = $3",
        )
        .bind(evenement_id)
        .bind(s.id)
        .bind(query.jour)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let souhaits: Vec<SouhaitDto> = sqlx::query_as(
            "SELECT commission_id, poste_id FROM souhait_commission WHERE soumission_id = $1",
        )
        .bind(s.id)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        out.push(BenevoleJourDto {
            soumission_id: s.id,
            nom: s.nom,
            prenom: s.prenom,
            permis_b: s.permis_b,
            polyvalent: s.polyvalent,
            creneau_ids,
            souhaits,
        });
    }

    Ok(Json(out))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AffectationRow {
    id: Uuid,
    soumission_id: Uuid,
    creneau_applicatif_id: Uuid,
    commission_id: Uuid,
    poste_id: Option<Uuid>,
    verrouille: bool,
}

pub async fn list_affectations(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Query(query): Query<JourQuery>,
) -> Result<Json<Vec<AffectationRow>>, StatusCode> {
    let rows: Vec<AffectationRow> = sqlx::query_as(
        "SELECT a.id, a.soumission_id, a.creneau_applicatif_id, a.commission_id, a.poste_id, \
                a.verrouille \
         FROM affectation a \
         JOIN creneau_applicatif c ON c.id = a.creneau_applicatif_id \
         WHERE c.evenement_id = $1 AND c.jour = $2",
    )
    .bind(evenement_id)
    .bind(query.jour)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct AffectationBody {
    pub soumission_id: Uuid,
    pub creneau_applicatif_id: Uuid,
    pub commission_id: Uuid,
    pub poste_id: Option<Uuid>,
}

impl From<AffectationBody> for CreateAffectationInput {
    fn from(b: AffectationBody) -> Self {
        CreateAffectationInput {
            soumission_id: b.soumission_id,
            creneau_applicatif_id: b.creneau_applicatif_id,
            commission_id: b.commission_id,
            poste_id: b.poste_id,
        }
    }
}

fn affectation_error_response(err: AffectationError) -> Response {
    match err {
        AffectationError::SoumissionEvenementMismatch
        | AffectationError::CreneauEvenementMismatch
        | AffectationError::CreneauCommissionMismatch
        | AffectationError::PosteNotInCommission
        | AffectationError::AlreadyAffecte => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        AffectationError::SoumissionNotFound
        | AffectationError::CreneauNotFound
        | AffectationError::CommissionNotFound
        | AffectationError::NotFound => StatusCode::NOT_FOUND.into_response(),
        AffectationError::Database(e) => {
            tracing::error!(error = %e, "affectation save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_affectation(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<AffectationBody>,
) -> Response {
    match create_affectation(&state.db, admin_id, evenement_id, body.into()).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(err) => affectation_error_response(err),
    }
}

pub async fn delete_affectation_endpoint(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path((evenement_id, affectation_id)): Path<(Uuid, Uuid)>,
) -> Response {
    match delete_affectation(&state.db, evenement_id, affectation_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => affectation_error_response(err),
    }
}
