use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthAdmin;
use crate::AppState;

use super::{add_responsable, remove_responsable, AddResponsableError, RemoveResponsableError};

#[derive(Serialize, sqlx::FromRow)]
pub struct ResponsableRow {
    commission_id: Uuid,
    soumission_id: Uuid,
    nom: String,
    prenom: String,
    created_at: DateTime<Utc>,
}

pub async fn list_responsables(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(evenement_id): Path<Uuid>,
) -> Result<Json<Vec<ResponsableRow>>, StatusCode> {
    let rows: Vec<ResponsableRow> = sqlx::query_as(
        "SELECT rc.commission_id, rc.soumission_id, s.nom, s.prenom, rc.created_at \
         FROM responsable_commission rc \
         JOIN soumission s ON s.id = rc.soumission_id \
         WHERE rc.evenement_id = $1 \
         ORDER BY rc.created_at",
    )
    .bind(evenement_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(rows))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SoumissionRow {
    id: Uuid,
    nom: String,
    prenom: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    q: String,
}

/// Recherche de bénévoles (soumissions) sur cet événement, pour le champ
/// "Ajouter un responsable" (pas d'écran "Bénévoles" dédié pour
/// fournir ça autrement, cf. HANDOFF). Résultats plafonnés, requête vide =
/// aucun résultat (évite de renvoyer toute la liste à l'ouverture).
pub async fn search_soumissions(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<SoumissionRow>>, StatusCode> {
    if params.q.trim().is_empty() {
        return Ok(Json(vec![]));
    }

    let pattern = format!("%{}%", params.q.trim());
    let rows: Vec<SoumissionRow> = sqlx::query_as(
        "SELECT id, nom, prenom FROM soumission \
         WHERE evenement_id = $1 AND (nom ILIKE $2 OR prenom ILIKE $2) \
         ORDER BY nom, prenom LIMIT 20",
    )
    .bind(evenement_id)
    .bind(pattern)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct AddResponsableBody {
    pub commission_id: Uuid,
    pub soumission_id: Uuid,
}

fn add_error_response(err: AddResponsableError) -> Response {
    match err {
        AddResponsableError::DejaResponsable
        | AddResponsableError::ChevauchementAvecAutreCommission(_, _) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        AddResponsableError::CommissionNotFound
        | AddResponsableError::BenevoleNotFound
        | AddResponsableError::BenevoleHorsEvenement => StatusCode::NOT_FOUND.into_response(),
        AddResponsableError::Database(e) => {
            tracing::error!(error = %e, "responsable save failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_responsable(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Path(evenement_id): Path<Uuid>,
    Json(body): Json<AddResponsableBody>,
) -> Response {
    match add_responsable(
        &state.db,
        admin_id,
        evenement_id,
        body.commission_id,
        body.soumission_id,
    )
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => add_error_response(err),
    }
}

pub async fn delete_responsable(
    State(state): State<AppState>,
    _admin: AuthAdmin,
    Path((evenement_id, commission_id, soumission_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Response {
    match remove_responsable(&state.db, evenement_id, commission_id, soumission_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(RemoveResponsableError::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(err @ RemoveResponsableError::Database(_)) => {
            tracing::error!(error = %err, "responsable delete failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
