//! Admin management (specs.md §11.4, wireframe s8): add/remove admins,
//! promote to superadmin. Reserved to superadmins (`auth::SuperAdmin`).

pub mod endpoint;

use uuid::Uuid;

pub struct NewAdmin {
    pub nom: String,
    pub prenom: String,
    pub email: String,
    pub is_superadmin: bool,
}

/// Kept well above any plausible real value — just enough to stop a
/// pasted-essay-length value from breaking the Admins screen's layout (cf.
/// the same limit on `Commission.nom`).
pub const NOM_MAX_LEN: usize = 100;
pub const EMAIL_MAX_LEN: usize = 255;

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("le nom, le prénom et l'email sont obligatoires")]
    MissingField,
    #[error("le nom et le prénom ne peuvent pas dépasser {NOM_MAX_LEN} caractères")]
    NomTooLong,
    #[error("l'email ne peut pas dépasser {EMAIL_MAX_LEN} caractères")]
    EmailTooLong,
    #[error("adresse email invalide")]
    InvalidEmail,
    #[error("un admin avec cet email existe déjà")]
    EmailTaken,
    #[error("admin introuvable")]
    NotFound,
    #[error("vous ne pouvez pas vous retirer vous-même")]
    CannotRemoveSelf,
    #[error("impossible de retirer le dernier superadmin")]
    LastSuperadmin,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err) if db_err.is_unique_violation())
}

/// Deliberately not RFC 5322 — just enough shape-checking to catch a typo
/// (missing "@", no domain) before it becomes an admin who can never log
/// in (the OTP email silently goes nowhere, specs.md §11.1).
fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !email.contains(char::is_whitespace)
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain.rsplit('.').next().is_some_and(|tld| tld.len() >= 2)
}

/// Creates a new admin (specs.md §11.4: "ajouter un admin ne demande pas de
/// mot de passe : il se connecte directement via le flux 11.1 une fois son
/// email enregistré").
pub async fn create_admin(pool: &sqlx::PgPool, input: NewAdmin) -> Result<Uuid, AdminError> {
    if input.nom.trim().is_empty()
        || input.prenom.trim().is_empty()
        || input.email.trim().is_empty()
    {
        return Err(AdminError::MissingField);
    }
    if input.nom.trim().chars().count() > NOM_MAX_LEN
        || input.prenom.trim().chars().count() > NOM_MAX_LEN
    {
        return Err(AdminError::NomTooLong);
    }
    if input.email.trim().chars().count() > EMAIL_MAX_LEN {
        return Err(AdminError::EmailTooLong);
    }
    if !is_valid_email(input.email.trim()) {
        return Err(AdminError::InvalidEmail);
    }

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO admins (nom, prenom, email, is_superadmin) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&input.nom)
    .bind(&input.prenom)
    .bind(&input.email)
    .bind(input.is_superadmin)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            AdminError::EmailTaken
        } else {
            AdminError::Database(e)
        }
    })?;

    Ok(id)
}

async fn superadmin_count(pool: &sqlx::PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM admins WHERE is_superadmin = true")
        .fetch_one(pool)
        .await
}

/// Promotes `id` to superadmin, or demotes it back to a regular admin —
/// refusing a demotion that would leave the app with no superadmin at all
/// (specs.md §11.4 doesn't say what happens then, and "nobody can manage
/// admins anymore" is clearly not it).
pub async fn set_superadmin(
    pool: &sqlx::PgPool,
    id: Uuid,
    is_superadmin: bool,
) -> Result<(), AdminError> {
    let current: bool = sqlx::query_scalar("SELECT is_superadmin FROM admins WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AdminError::NotFound)?;

    if current && !is_superadmin && superadmin_count(pool).await? <= 1 {
        return Err(AdminError::LastSuperadmin);
    }

    sqlx::query("UPDATE admins SET is_superadmin = $1 WHERE id = $2")
        .bind(is_superadmin)
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Removes an admin (specs.md §11.4). Refuses to remove the caller
/// themselves (self-lockout) or the last remaining superadmin. Sessions,
/// and the `created_by`/`updated_by` audit columns referencing this admin,
/// are handled by the FK `ON DELETE CASCADE`/`SET NULL` set up in
/// `20260101000006_admin_delete_fk.sql`.
pub async fn delete_admin(
    pool: &sqlx::PgPool,
    actor_id: Uuid,
    target_id: Uuid,
) -> Result<(), AdminError> {
    if actor_id == target_id {
        return Err(AdminError::CannotRemoveSelf);
    }

    let target_is_superadmin: bool =
        sqlx::query_scalar("SELECT is_superadmin FROM admins WHERE id = $1")
            .bind(target_id)
            .fetch_optional(pool)
            .await?
            .ok_or(AdminError::NotFound)?;

    if target_is_superadmin && superadmin_count(pool).await? <= 1 {
        return Err(AdminError::LastSuperadmin);
    }

    sqlx::query("DELETE FROM admins WHERE id = $1")
        .bind(target_id)
        .execute(pool)
        .await?;

    Ok(())
}
