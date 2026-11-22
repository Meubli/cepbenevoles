use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::SuperAdmin;
use crate::AppState;

use super::{create_admin, delete_admin, set_superadmin, AdminError, NewAdmin};

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminDto {
    id: Uuid,
    nom: Option<String>,
    prenom: Option<String>,
    email: String,
    is_superadmin: bool,
    created_at: DateTime<Utc>,
}

pub async fn list_admins(
    State(state): State<AppState>,
    _superadmin: SuperAdmin,
) -> Result<Json<Vec<AdminDto>>, StatusCode> {
    let rows: Vec<AdminDto> = sqlx::query_as(
        "SELECT id, nom, prenom, email, is_superadmin, created_at \
         FROM admins ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct NewAdminBody {
    pub nom: String,
    pub prenom: String,
    pub email: String,
    pub is_superadmin: bool,
}

fn admin_error_response(err: AdminError) -> Response {
    match err {
        AdminError::MissingField | AdminError::EmailTaken => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        AdminError::NotFound => StatusCode::NOT_FOUND.into_response(),
        AdminError::CannotRemoveSelf | AdminError::LastSuperadmin => (
            StatusCode::CONFLICT,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
        AdminError::Database(e) => {
            tracing::error!(error = %e, "admin management query failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_admin(
    State(state): State<AppState>,
    _superadmin: SuperAdmin,
    Json(body): Json<NewAdminBody>,
) -> Response {
    let input = NewAdmin {
        nom: body.nom,
        prenom: body.prenom,
        email: body.email,
        is_superadmin: body.is_superadmin,
    };
    match create_admin(&state.db, input).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(err) => admin_error_response(err),
    }
}

#[derive(Deserialize)]
pub struct SetSuperadminBody {
    pub is_superadmin: bool,
}

pub async fn patch_admin(
    State(state): State<AppState>,
    _superadmin: SuperAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<SetSuperadminBody>,
) -> Response {
    match set_superadmin(&state.db, id, body.is_superadmin).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => admin_error_response(err),
    }
}

pub async fn delete_admin_endpoint(
    State(state): State<AppState>,
    SuperAdmin(actor_id): SuperAdmin,
    Path(id): Path<Uuid>,
) -> Response {
    match delete_admin(&state.db, actor_id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => admin_error_response(err),
    }
}
