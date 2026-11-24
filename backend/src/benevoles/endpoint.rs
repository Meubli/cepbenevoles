use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthAdmin;
use crate::AppState;

use super::{
    create_benevole, CreateBenevoleError, NouveauBenevoleInput, SouhaitInput, SoumissionInput,
};

#[derive(Deserialize)]
pub struct SouhaitBody {
    pub commission_id: Uuid,
    pub poste_id: Option<Uuid>,
}

impl From<SouhaitBody> for SouhaitInput {
    fn from(b: SouhaitBody) -> Self {
        SouhaitInput {
            commission_id: b.commission_id,
            poste_id: b.poste_id,
        }
    }
}

#[derive(Deserialize)]
pub struct DisponibiliteBody {
    pub jour: NaiveDate,
    pub slot: String,
}

#[derive(Deserialize)]
pub struct SoumissionBody {
    pub evenement_id: Uuid,
    #[serde(default)]
    pub disponibilites: Vec<DisponibiliteBody>,
    #[serde(default)]
    pub polyvalent: bool,
    #[serde(default)]
    pub souhaits: Vec<SouhaitBody>,
}

impl From<SoumissionBody> for SoumissionInput {
    fn from(b: SoumissionBody) -> Self {
        SoumissionInput {
            evenement_id: b.evenement_id,
            disponibilites: b
                .disponibilites
                .into_iter()
                .map(|d| (d.jour, d.slot))
                .collect(),
            polyvalent: b.polyvalent,
            souhaits: b.souhaits.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Deserialize)]
pub struct NouveauBenevoleBody {
    pub nom: String,
    pub prenom: String,
    pub email: String,
    pub telephone: String,
    pub tranche_age: String,
    pub permis_b: bool,
    pub taille_tshirt: String,
    pub soumissions: Vec<SoumissionBody>,
}

impl From<NouveauBenevoleBody> for NouveauBenevoleInput {
    fn from(b: NouveauBenevoleBody) -> Self {
        NouveauBenevoleInput {
            nom: b.nom,
            prenom: b.prenom,
            email: b.email,
            telephone: b.telephone,
            tranche_age: b.tranche_age,
            permis_b: b.permis_b,
            taille_tshirt: b.taille_tshirt,
            soumissions: b.soumissions.into_iter().map(Into::into).collect(),
        }
    }
}

fn unprocessable(err: &CreateBenevoleError, field: Option<&str>) -> Response {
    let mut body = json!({"error": err.to_string()});
    if let Some(field) = field {
        body["field"] = json!(field);
    }
    (StatusCode::UNPROCESSABLE_ENTITY, Json(body)).into_response()
}

fn create_error_response(err: CreateBenevoleError) -> Response {
    // `field` names which input the message applies to (cf. admins/endpoint.rs),
    // so the frontend can show it right under that field instead of only in
    // the screen's top-level error banner.
    match err {
        CreateBenevoleError::EmptyNom | CreateBenevoleError::NomTooLong => {
            unprocessable(&err, Some("nom"))
        }
        CreateBenevoleError::EmptyPrenom | CreateBenevoleError::PrenomTooLong => {
            unprocessable(&err, Some("prenom"))
        }
        CreateBenevoleError::InvalidEmail | CreateBenevoleError::EmailTooLong => {
            unprocessable(&err, Some("email"))
        }
        CreateBenevoleError::EmptyTelephone
        | CreateBenevoleError::TelephoneTooLong
        | CreateBenevoleError::InvalidTelephone => unprocessable(&err, Some("telephone")),
        CreateBenevoleError::InvalidTrancheAge => unprocessable(&err, Some("tranche_age")),
        CreateBenevoleError::EmptyTailleTshirt | CreateBenevoleError::TailleTshirtTooLong => {
            unprocessable(&err, Some("taille_tshirt"))
        }
        CreateBenevoleError::NoEvenement
        | CreateBenevoleError::DuplicateEvenement
        | CreateBenevoleError::JourHorsEvenement
        | CreateBenevoleError::InvalidSlot
        | CreateBenevoleError::PosteNotInCommission
        | CreateBenevoleError::EmptySoumission => unprocessable(&err, None),
        CreateBenevoleError::EvenementNotFound | CreateBenevoleError::CommissionNotFound => {
            StatusCode::NOT_FOUND.into_response()
        }
        CreateBenevoleError::Database(e) => {
            tracing::error!(error = %e, "benevole creation failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn post_benevole(
    State(state): State<AppState>,
    AuthAdmin(admin_id): AuthAdmin,
    Json(body): Json<NouveauBenevoleBody>,
) -> Response {
    match create_benevole(&state.db, admin_id, body.into()).await {
        Ok(ids) => (StatusCode::CREATED, Json(json!({"soumission_ids": ids}))).into_response(),
        Err(err) => create_error_response(err),
    }
}
