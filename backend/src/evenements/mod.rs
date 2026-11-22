//! Événements CRUD (specs.md §3.3, wireframe s11): create/edit/archive, with
//! live `sheet_id` validation against the real Sheets API.

pub mod endpoint;

use uuid::Uuid;

use crate::google_auth::AuthError;
use crate::sheets::SheetsError;
use crate::SyncContext;

#[derive(Debug, thiserror::Error)]
pub enum SheetIdError {
    #[error("Cette Sheet n'existe pas ou son ID est incorrect")]
    NotFound,
    #[error("Sheet trouvée mais non partagée avec le compte de service — partagez-la en lecture d'abord")]
    Forbidden,
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    #[error("sheets error: {0}")]
    Sheets(#[from] SheetsError),
}

/// Validates a `sheet_id` against the real Google Sheets API before an
/// `Evenement` is saved with it (specs.md §3.3). An empty `sheet_id` is
/// always accepted without a network call (field is optional). When no
/// `SyncContext` is configured (local dev, e2e — no Google credentials),
/// validation is skipped entirely rather than blocking event creation in
/// every environment that lacks real Google credentials.
pub async fn validate_sheet_id(
    sync: Option<&SyncContext>,
    sheet_id: &str,
) -> Result<(), SheetIdError> {
    if sheet_id.is_empty() {
        return Ok(());
    }
    let Some(sync) = sync else {
        tracing::warn!("sheet_id validation skipped: no SyncContext configured");
        return Ok(());
    };

    let token = sync.auth.access_token().await?;
    // A minimal, cheap read — just proving the sheet exists and is
    // reachable, not fetching real data.
    sync.sheets
        .fetch_values(&token, sheet_id, "A1:A1")
        .await
        .map_err(|err| match err {
            SheetsError::NotFound => SheetIdError::NotFound,
            SheetsError::Forbidden => SheetIdError::Forbidden,
            other => SheetIdError::Sheets(other),
        })?;

    Ok(())
}

pub struct EvenementInput {
    pub nom: String,
    pub date_debut: chrono::NaiveDate,
    pub date_fin: chrono::NaiveDate,
    pub lieu: Option<String>,
    pub sheet_id: Option<String>,
    pub bloc_formulaire: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveEvenementError {
    #[error("date_fin doit être postérieure ou égale à date_debut")]
    InvalidDates,
    #[error(transparent)]
    SheetId(#[from] SheetIdError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn create_evenement(
    pool: &sqlx::PgPool,
    sync: Option<&SyncContext>,
    admin_id: Uuid,
    input: EvenementInput,
) -> Result<Uuid, SaveEvenementError> {
    if input.date_fin < input.date_debut {
        return Err(SaveEvenementError::InvalidDates);
    }
    validate_sheet_id(sync, input.sheet_id.as_deref().unwrap_or("")).await?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO evenement \
            (nom, date_debut, date_fin, lieu, sheet_id, bloc_formulaire, created_by, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7) \
         RETURNING id",
    )
    .bind(&input.nom)
    .bind(input.date_debut)
    .bind(input.date_fin)
    .bind(&input.lieu)
    .bind(input.sheet_id.filter(|s| !s.is_empty()))
    .bind(input.bloc_formulaire.filter(|s| !s.is_empty()))
    .bind(admin_id)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn update_evenement(
    pool: &sqlx::PgPool,
    sync: Option<&SyncContext>,
    admin_id: Uuid,
    id: Uuid,
    input: EvenementInput,
) -> Result<(), SaveEvenementError> {
    if input.date_fin < input.date_debut {
        return Err(SaveEvenementError::InvalidDates);
    }
    validate_sheet_id(sync, input.sheet_id.as_deref().unwrap_or("")).await?;

    sqlx::query(
        "UPDATE evenement SET \
            nom = $1, date_debut = $2, date_fin = $3, lieu = $4, \
            sheet_id = $5, bloc_formulaire = $6, updated_by = $7, updated_at = now() \
         WHERE id = $8",
    )
    .bind(&input.nom)
    .bind(input.date_debut)
    .bind(input.date_fin)
    .bind(&input.lieu)
    .bind(input.sheet_id.filter(|s| !s.is_empty()))
    .bind(input.bloc_formulaire.filter(|s| !s.is_empty()))
    .bind(admin_id)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn set_archive(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    id: Uuid,
    archive: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE evenement SET archive = $1, updated_by = $2, updated_at = now() WHERE id = $3",
    )
    .bind(archive)
    .bind(admin_id)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
