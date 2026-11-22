//! Commission/Poste referential CRUD (specs.md §5, wireframe s5). Scope
//! chosen with the user: create/edit a commission's fields and its postes
//! (add/remove, no reordering — `poste` has no order column), no
//! activation/deactivation toggle (the `actif` column stays migration-only
//! for now).

pub mod endpoint;

use uuid::Uuid;

pub struct CommissionInput {
    pub nom: String,
    pub prerequis_libre_texte: Option<String>,
    pub a_des_postes: bool,
    pub grille_creneaux: String,
    pub besoin_chiffre: bool,
    pub couleur: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveCommissionError {
    #[error("le nom de la commission ne peut pas être vide")]
    EmptyNom,
    #[error("une commission nommée \"{0}\" existe déjà")]
    DuplicateNom(String),
    #[error("grille_creneaux doit être \"generale\" ou \"propre\"")]
    InvalidGrille,
    #[error("couleur invalide — attendu un hex #rrggbb")]
    InvalidCouleur,
    #[error("commission introuvable")]
    NotFound,
    #[error(
        "retirez d'abord tous les postes de cette commission avant de désactiver \"a des postes\""
    )]
    PostesRemain,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err) if db_err.is_unique_violation())
}

fn is_foreign_key_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation())
}

fn validate_couleur(couleur: &str) -> bool {
    couleur.len() == 7
        && couleur.starts_with('#')
        && couleur[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn validate_grille(grille: &str) -> bool {
    grille == "generale" || grille == "propre"
}

fn validate_input(input: &CommissionInput) -> Result<(), SaveCommissionError> {
    if input.nom.trim().is_empty() {
        return Err(SaveCommissionError::EmptyNom);
    }
    if !validate_grille(&input.grille_creneaux) {
        return Err(SaveCommissionError::InvalidGrille);
    }
    if !validate_couleur(&input.couleur) {
        return Err(SaveCommissionError::InvalidCouleur);
    }
    Ok(())
}

pub async fn create_commission(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    input: CommissionInput,
) -> Result<Uuid, SaveCommissionError> {
    validate_input(&input)?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO commission \
            (nom, prerequis_libre_texte, a_des_postes, grille_creneaux, besoin_chiffre, \
             couleur, created_by, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7) \
         RETURNING id",
    )
    .bind(input.nom.trim())
    .bind(&input.prerequis_libre_texte)
    .bind(input.a_des_postes)
    .bind(&input.grille_creneaux)
    .bind(input.besoin_chiffre)
    .bind(&input.couleur)
    .bind(admin_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            SaveCommissionError::DuplicateNom(input.nom.trim().to_string())
        } else {
            SaveCommissionError::Database(e)
        }
    })?;

    Ok(id)
}

pub async fn update_commission(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    id: Uuid,
    input: CommissionInput,
) -> Result<(), SaveCommissionError> {
    validate_input(&input)?;

    if !input.a_des_postes {
        let poste_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM poste WHERE commission_id = $1")
                .bind(id)
                .fetch_one(pool)
                .await?;
        if poste_count > 0 {
            return Err(SaveCommissionError::PostesRemain);
        }
    }

    let rows_affected = sqlx::query(
        "UPDATE commission SET \
            nom = $1, prerequis_libre_texte = $2, a_des_postes = $3, grille_creneaux = $4, \
            besoin_chiffre = $5, couleur = $6, updated_by = $7, updated_at = now() \
         WHERE id = $8",
    )
    .bind(input.nom.trim())
    .bind(&input.prerequis_libre_texte)
    .bind(input.a_des_postes)
    .bind(&input.grille_creneaux)
    .bind(input.besoin_chiffre)
    .bind(&input.couleur)
    .bind(admin_id)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            SaveCommissionError::DuplicateNom(input.nom.trim().to_string())
        } else {
            SaveCommissionError::Database(e)
        }
    })?
    .rows_affected();

    if rows_affected == 0 {
        return Err(SaveCommissionError::NotFound);
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum PosteError {
    #[error("le nom du poste ne peut pas être vide")]
    EmptyNom,
    #[error("commission introuvable")]
    CommissionNotFound,
    #[error("un poste nommé \"{0}\" existe déjà pour cette commission")]
    DuplicateNom(String),
    #[error("poste introuvable")]
    NotFound,
    #[error(
        "ce poste est déjà utilisé (mapping ou souhait existant) et ne peut pas être supprimé"
    )]
    InUse,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn add_poste(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    commission_id: Uuid,
    nom: String,
) -> Result<Uuid, PosteError> {
    if nom.trim().is_empty() {
        return Err(PosteError::EmptyNom);
    }

    let commission_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM commission WHERE id = $1)")
            .bind(commission_id)
            .fetch_one(pool)
            .await?;
    if !commission_exists {
        return Err(PosteError::CommissionNotFound);
    }

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO poste (commission_id, nom, created_by, updated_by) \
         VALUES ($1, $2, $3, $3) RETURNING id",
    )
    .bind(commission_id)
    .bind(nom.trim())
    .bind(admin_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            PosteError::DuplicateNom(nom.trim().to_string())
        } else {
            PosteError::Database(e)
        }
    })?;

    Ok(id)
}

pub async fn remove_poste(
    pool: &sqlx::PgPool,
    commission_id: Uuid,
    poste_id: Uuid,
) -> Result<(), PosteError> {
    let rows_affected = sqlx::query("DELETE FROM poste WHERE id = $1 AND commission_id = $2")
        .bind(poste_id)
        .bind(commission_id)
        .execute(pool)
        .await
        .map_err(|e| {
            if is_foreign_key_violation(&e) {
                PosteError::InUse
            } else {
                PosteError::Database(e)
            }
        })?
        .rows_affected();

    if rows_affected == 0 {
        return Err(PosteError::NotFound);
    }

    Ok(())
}
