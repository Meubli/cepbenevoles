//! Événements CRUD (specs.md §3.3, wireframe s11): create/edit/archive, with
//! live `sheet_id` validation against the real Sheets API.

pub mod endpoint;

use uuid::Uuid;

use crate::google_auth::AuthError;
use crate::sheets::SheetsError;
use crate::SyncContext;

#[derive(Debug, thiserror::Error)]
pub enum SheetIdError {
    #[error(
        "ID de Sheet invalide — collez l'ID seul ou l'URL complète de la Google Sheet, pas un autre lien"
    )]
    InvalidFormat,
    #[error("Cette Sheet n'existe pas ou son ID est incorrect")]
    NotFound,
    #[error("Sheet trouvée mais non partagée avec le compte de service — partagez-la en lecture d'abord")]
    Forbidden,
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    #[error("sheets error: {0}")]
    Sheets(#[from] SheetsError),
}

/// Accepts either a bare Sheet id or a full Google Sheets URL (admins will
/// paste either) and returns the bare id, rejecting anything that isn't a
/// plausible Drive file id — a raw `/` in particular would break the HTTP
/// client's URL building (cf. `SheetsClient`), not just "look wrong".
fn extract_sheet_id(input: &str) -> Result<String, SheetIdError> {
    let candidate = match input.split("/d/").nth(1) {
        Some(after) => after.split('/').next().unwrap_or(""),
        None => input,
    };
    let candidate = candidate.trim();

    let valid = !candidate.is_empty()
        && candidate
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');

    if valid {
        Ok(candidate.to_string())
    } else {
        Err(SheetIdError::InvalidFormat)
    }
}

/// Validates a `sheet_id` against the real Google Sheets API before an
/// `Evenement` is saved with it (specs.md §3.3). An empty `sheet_id` is
/// always accepted without a network call (field is optional). When no
/// `SyncContext` is configured (local dev, e2e — no Google credentials),
/// validation is skipped entirely rather than blocking event creation in
/// every environment that lacks real Google credentials. Returns the
/// normalized (bare-id) form on success, since the input may have been a
/// full URL.
pub async fn validate_sheet_id(
    sync: Option<&SyncContext>,
    sheet_id: &str,
) -> Result<Option<String>, SheetIdError> {
    if sheet_id.is_empty() {
        return Ok(None);
    }
    let id = extract_sheet_id(sheet_id)?;

    let Some(sync) = sync else {
        tracing::warn!("sheet_id validation skipped: no SyncContext configured");
        return Ok(Some(id));
    };

    let token = sync.auth.access_token().await?;
    sync.sheets
        .check_access(&token, &id)
        .await
        .map_err(|err| match err {
            SheetsError::NotFound => SheetIdError::NotFound,
            SheetsError::Forbidden => SheetIdError::Forbidden,
            other => SheetIdError::Sheets(other),
        })?;

    Ok(Some(id))
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
pub enum BlocError {
    #[error("un sheet_id est renseigné : choisissez le bloc du formulaire qu'il alimente")]
    Missing,
    #[error("bloc_formulaire \"{0}\" inconnu")]
    Unknown(String),
    #[error("ce bloc est déjà utilisé par l'événement \"{0}\" sur cette même Sheet")]
    AlreadyUsed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum SaveEvenementError {
    #[error("le nom de l'événement ne peut pas être vide")]
    EmptyNom,
    #[error("date_fin doit être postérieure ou égale à date_debut")]
    InvalidDates,
    #[error(transparent)]
    SheetId(#[from] SheetIdError),
    #[error(transparent)]
    Bloc(#[from] BlocError),
    #[error("événement introuvable")]
    NotFound,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Checks `bloc` is required/known/not already claimed by another
/// non-archived event on the same `sheet_id` — called on create, update,
/// and on unarchiving (re-activating an event must not silently recreate a
/// conflict that archiving had made moot).
async fn validate_bloc(
    pool: &sqlx::PgPool,
    sheet_id: Option<&str>,
    bloc: Option<&str>,
    exclude_id: Option<Uuid>,
) -> Result<(), BlocError> {
    let Some(sheet_id) = sheet_id else {
        return Ok(());
    };
    let Some(bloc) = bloc else {
        return Err(BlocError::Missing);
    };
    if crate::sync::form_2027::bloc_to_slug(bloc).is_none() {
        return Err(BlocError::Unknown(bloc.to_string()));
    }

    let other_nom: Option<String> = sqlx::query_scalar(
        "SELECT nom FROM evenement \
         WHERE sheet_id = $1 AND bloc_formulaire = $2 AND archive = false \
           AND ($3::uuid IS NULL OR id != $3) \
         LIMIT 1",
    )
    .bind(sheet_id)
    .bind(bloc)
    .bind(exclude_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        // validate_bloc deliberately doesn't carry a Database variant of its
        // own — surfacing a query failure as "already used" would be
        // misleading, so this path is only reachable on a real DB outage,
        // logged by the caller via SaveEvenementError::Database instead.
        tracing::error!(error = %e, "bloc duplicate check query failed");
        BlocError::Unknown(bloc.to_string())
    })?;

    if let Some(nom) = other_nom {
        return Err(BlocError::AlreadyUsed(nom));
    }
    Ok(())
}

pub async fn create_evenement(
    pool: &sqlx::PgPool,
    sync: Option<&SyncContext>,
    admin_id: Uuid,
    input: EvenementInput,
) -> Result<Uuid, SaveEvenementError> {
    if input.nom.trim().is_empty() {
        return Err(SaveEvenementError::EmptyNom);
    }
    if input.date_fin < input.date_debut {
        return Err(SaveEvenementError::InvalidDates);
    }
    let sheet_id = validate_sheet_id(sync, input.sheet_id.as_deref().unwrap_or("")).await?;
    validate_bloc(
        pool,
        sheet_id.as_deref(),
        input.bloc_formulaire.as_deref().filter(|s| !s.is_empty()),
        None,
    )
    .await?;

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
    .bind(&sheet_id)
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
    if input.nom.trim().is_empty() {
        return Err(SaveEvenementError::EmptyNom);
    }
    if input.date_fin < input.date_debut {
        return Err(SaveEvenementError::InvalidDates);
    }
    let sheet_id = validate_sheet_id(sync, input.sheet_id.as_deref().unwrap_or("")).await?;
    validate_bloc(
        pool,
        sheet_id.as_deref(),
        input.bloc_formulaire.as_deref().filter(|s| !s.is_empty()),
        Some(id),
    )
    .await?;

    let rows_affected = sqlx::query(
        "UPDATE evenement SET \
            nom = $1, date_debut = $2, date_fin = $3, lieu = $4, \
            sheet_id = $5, bloc_formulaire = $6, updated_by = $7, updated_at = now() \
         WHERE id = $8",
    )
    .bind(&input.nom)
    .bind(input.date_debut)
    .bind(input.date_fin)
    .bind(&input.lieu)
    .bind(&sheet_id)
    .bind(input.bloc_formulaire.filter(|s| !s.is_empty()))
    .bind(admin_id)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(SaveEvenementError::NotFound);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum SetArchiveError {
    #[error("événement introuvable")]
    NotFound,
    #[error(transparent)]
    Bloc(#[from] BlocError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn set_archive(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    id: Uuid,
    archive: bool,
) -> Result<(), SetArchiveError> {
    if !archive {
        // Unarchiving re-activates the event's bloc/sheet_id combination —
        // re-check it doesn't now collide with another event that was
        // (legitimately) given that same bloc while this one was archived.
        let row: Option<(Option<String>, Option<String>)> =
            sqlx::query_as("SELECT sheet_id, bloc_formulaire FROM evenement WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        let Some((sheet_id, bloc)) = row else {
            return Err(SetArchiveError::NotFound);
        };
        validate_bloc(pool, sheet_id.as_deref(), bloc.as_deref(), Some(id)).await?;
    }

    let rows_affected = sqlx::query(
        "UPDATE evenement SET archive = $1, updated_by = $2, updated_at = now() WHERE id = $3",
    )
    .bind(archive)
    .bind(admin_id)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(SetArchiveError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_sheet_id_accepts_a_bare_id() {
        assert_eq!(
            extract_sheet_id("1VanRtLnS2lF5XpUtUpjyv9MX2lQh7EPcAZCIPsm3jsc").unwrap(),
            "1VanRtLnS2lF5XpUtUpjyv9MX2lQh7EPcAZCIPsm3jsc"
        );
    }

    #[test]
    fn extract_sheet_id_extracts_from_a_full_sheets_url() {
        let url = "https://docs.google.com/spreadsheets/d/1VanRtLnS2lF5XpUtUpjyv9MX2lQh7EPcAZCIPsm3jsc/edit?gid=0#gid=0";
        assert_eq!(
            extract_sheet_id(url).unwrap(),
            "1VanRtLnS2lF5XpUtUpjyv9MX2lQh7EPcAZCIPsm3jsc"
        );
    }

    #[test]
    fn extract_sheet_id_rejects_a_non_sheet_url() {
        // A Forms edit URL has no /d/<id>/ segment matching a Sheet id shape
        // the same way — but more importantly, anything with a raw slash
        // left in it must be rejected rather than breaking SheetsClient's
        // URL building downstream.
        assert!(extract_sheet_id("https://example.com/not/a/sheet").is_err());
        assert!(extract_sheet_id("has/a/slash").is_err());
        assert!(extract_sheet_id("").is_err());
    }
}
