//! Écran Planning (specs.md §6/§7.1) — ce lot couvre uniquement le schéma
//! et le CRUD de l'affectation manuelle. §6.3 : toute affectation posée ici
//! est marquée verrouillée (toujours `true` — pas de génération
//! automatique encore, cf. §6.2, non construite dans ce lot).

pub mod endpoint;

use uuid::Uuid;

pub struct CreateAffectationInput {
    pub soumission_id: Uuid,
    pub creneau_applicatif_id: Uuid,
    pub commission_id: Uuid,
    pub poste_id: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum AffectationError {
    #[error("soumission introuvable")]
    SoumissionNotFound,
    #[error("cette soumission n'appartient pas à cet événement")]
    SoumissionEvenementMismatch,
    #[error("créneau introuvable")]
    CreneauNotFound,
    #[error("ce créneau n'appartient pas à cet événement")]
    CreneauEvenementMismatch,
    #[error("commission introuvable")]
    CommissionNotFound,
    #[error("ce créneau n'appartient pas à la grille de cette commission")]
    CreneauCommissionMismatch,
    #[error("ce poste n'appartient pas à la commission choisie")]
    PosteNotInCommission,
    #[error("ce bénévole est déjà affecté sur ce créneau — retirez l'affectation existante avant d'en poser une autre")]
    AlreadyAffecte,
    #[error("affectation introuvable")]
    NotFound,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn create_affectation(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    input: CreateAffectationInput,
) -> Result<Uuid, AffectationError> {
    let soumission_evenement_id: Option<Uuid> =
        sqlx::query_scalar("SELECT evenement_id FROM soumission WHERE id = $1")
            .bind(input.soumission_id)
            .fetch_optional(pool)
            .await?;
    match soumission_evenement_id {
        None => return Err(AffectationError::SoumissionNotFound),
        Some(id) if id != evenement_id => {
            return Err(AffectationError::SoumissionEvenementMismatch)
        }
        Some(_) => {}
    }

    let creneau: Option<(Uuid, Option<Uuid>)> =
        sqlx::query_as("SELECT evenement_id, commission_id FROM creneau_applicatif WHERE id = $1")
            .bind(input.creneau_applicatif_id)
            .fetch_optional(pool)
            .await?;
    let (creneau_evenement_id, creneau_commission_id) = match creneau {
        None => return Err(AffectationError::CreneauNotFound),
        Some(c) => c,
    };
    if creneau_evenement_id != evenement_id {
        return Err(AffectationError::CreneauEvenementMismatch);
    }

    let grille_propre: Option<bool> =
        sqlx::query_scalar("SELECT grille_creneaux = 'propre' FROM commission WHERE id = $1")
            .bind(input.commission_id)
            .fetch_optional(pool)
            .await?;
    let grille_propre = match grille_propre {
        None => return Err(AffectationError::CommissionNotFound),
        Some(g) => g,
    };
    let creneau_matches = if grille_propre {
        creneau_commission_id == Some(input.commission_id)
    } else {
        creneau_commission_id.is_none()
    };
    if !creneau_matches {
        return Err(AffectationError::CreneauCommissionMismatch);
    }

    if let Some(poste_id) = input.poste_id {
        let belongs: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM poste WHERE id = $1 AND commission_id = $2)",
        )
        .bind(poste_id)
        .bind(input.commission_id)
        .fetch_one(pool)
        .await?;
        if !belongs {
            return Err(AffectationError::PosteNotInCommission);
        }
    }

    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM affectation \
         WHERE soumission_id = $1 AND creneau_applicatif_id = $2)",
    )
    .bind(input.soumission_id)
    .bind(input.creneau_applicatif_id)
    .fetch_one(pool)
    .await?;
    if already {
        return Err(AffectationError::AlreadyAffecte);
    }

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO affectation \
            (soumission_id, creneau_applicatif_id, commission_id, poste_id, verrouille, \
             created_by, updated_by) \
         VALUES ($1, $2, $3, $4, true, $5, $5) RETURNING id",
    )
    .bind(input.soumission_id)
    .bind(input.creneau_applicatif_id)
    .bind(input.commission_id)
    .bind(input.poste_id)
    .bind(admin_id)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn delete_affectation(
    pool: &sqlx::PgPool,
    evenement_id: Uuid,
    affectation_id: Uuid,
) -> Result<(), AffectationError> {
    let rows_affected = sqlx::query(
        "DELETE FROM affectation USING creneau_applicatif \
         WHERE affectation.id = $1 \
           AND affectation.creneau_applicatif_id = creneau_applicatif.id \
           AND creneau_applicatif.evenement_id = $2",
    )
    .bind(affectation_id)
    .bind(evenement_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(AffectationError::NotFound);
    }
    Ok(())
}
