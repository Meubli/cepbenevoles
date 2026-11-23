//! Besoins par commission × créneau, et par poste × créneau pour les
//! commissions qui en ont (specs.md §5, §5.1, wireframe s3). Une commission
//! "sans besoin chiffré" (§5.3) est filtrée côté application — ni saisie ni
//! lecture ici pour elle.

pub mod endpoint;

use uuid::Uuid;

pub struct BesoinCommissionInput {
    pub creneau_applicatif_id: Uuid,
    pub commission_id: Uuid,
    pub nb_requis: i32,
}

pub struct BesoinPosteInput {
    pub creneau_applicatif_id: Uuid,
    pub poste_id: Uuid,
    pub nb_requis: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum BesoinCommissionError {
    #[error("le nombre requis ne peut pas être négatif")]
    NbRequisNegatif,
    #[error("commission introuvable")]
    CommissionNotFound,
    #[error("cette commission est \"sans besoin chiffré\" — pas de saisie possible")]
    CommissionSansBesoinChiffre,
    #[error("créneau introuvable pour cet événement")]
    CreneauNotFound,
    #[error("ce créneau n'appartient pas à la grille de cette commission")]
    CreneauHorsPerimetre,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum BesoinPosteError {
    #[error("le nombre requis ne peut pas être négatif")]
    NbRequisNegatif,
    #[error("poste introuvable")]
    PosteNotFound,
    #[error("créneau introuvable pour cet événement")]
    CreneauNotFound,
    #[error("ce créneau n'appartient pas à la grille de cette commission")]
    CreneauHorsPerimetre,
    #[error(
        "la somme des besoins par poste sur ce créneau ({0}) dépasserait le besoin de la commission ({1})"
    )]
    DepasseBesoinCommission(i32, i32),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(sqlx::FromRow)]
struct CommissionRow {
    besoin_chiffre: bool,
    grille_creneaux: String,
}

#[derive(sqlx::FromRow)]
struct CreneauRow {
    commission_id: Option<Uuid>,
}

/// A commission's own grid is its própre créneaux (`commission_id` =
/// this commission) if it has `grille_creneaux = 'propre'`, otherwise the
/// grille générale (`commission_id IS NULL`) — same rule as the Créneaux
/// screen (§4.3).
fn creneau_in_commission_scope(creneau: &CreneauRow, commission_id: Uuid, propre: bool) -> bool {
    if propre {
        creneau.commission_id == Some(commission_id)
    } else {
        creneau.commission_id.is_none()
    }
}

pub async fn upsert_besoin_commission(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    input: BesoinCommissionInput,
) -> Result<(), BesoinCommissionError> {
    if input.nb_requis < 0 {
        return Err(BesoinCommissionError::NbRequisNegatif);
    }

    let commission: Option<CommissionRow> =
        sqlx::query_as("SELECT besoin_chiffre, grille_creneaux FROM commission WHERE id = $1")
            .bind(input.commission_id)
            .fetch_optional(pool)
            .await?;
    let Some(commission) = commission else {
        return Err(BesoinCommissionError::CommissionNotFound);
    };
    if !commission.besoin_chiffre {
        return Err(BesoinCommissionError::CommissionSansBesoinChiffre);
    }

    let creneau: Option<CreneauRow> = sqlx::query_as(
        "SELECT commission_id FROM creneau_applicatif WHERE id = $1 AND evenement_id = $2",
    )
    .bind(input.creneau_applicatif_id)
    .bind(evenement_id)
    .fetch_optional(pool)
    .await?;
    let Some(creneau) = creneau else {
        return Err(BesoinCommissionError::CreneauNotFound);
    };
    if !creneau_in_commission_scope(
        &creneau,
        input.commission_id,
        commission.grille_creneaux == "propre",
    ) {
        return Err(BesoinCommissionError::CreneauHorsPerimetre);
    }

    sqlx::query(
        "INSERT INTO besoin_commission \
            (evenement_id, creneau_applicatif_id, commission_id, nb_requis, created_by, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $5) \
         ON CONFLICT (creneau_applicatif_id, commission_id) \
         DO UPDATE SET nb_requis = $4, updated_by = $5, updated_at = now()",
    )
    .bind(evenement_id)
    .bind(input.creneau_applicatif_id)
    .bind(input.commission_id)
    .bind(input.nb_requis)
    .bind(admin_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn upsert_besoin_poste(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    input: BesoinPosteInput,
) -> Result<(), BesoinPosteError> {
    if input.nb_requis < 0 {
        return Err(BesoinPosteError::NbRequisNegatif);
    }

    let commission_id: Option<Uuid> =
        sqlx::query_scalar("SELECT commission_id FROM poste WHERE id = $1")
            .bind(input.poste_id)
            .fetch_optional(pool)
            .await?;
    let Some(commission_id) = commission_id else {
        return Err(BesoinPosteError::PosteNotFound);
    };

    let grille_creneaux: Option<String> =
        sqlx::query_scalar("SELECT grille_creneaux FROM commission WHERE id = $1")
            .bind(commission_id)
            .fetch_optional(pool)
            .await?;
    let propre = grille_creneaux.as_deref() == Some("propre");

    let creneau: Option<CreneauRow> = sqlx::query_as(
        "SELECT commission_id FROM creneau_applicatif WHERE id = $1 AND evenement_id = $2",
    )
    .bind(input.creneau_applicatif_id)
    .bind(evenement_id)
    .fetch_optional(pool)
    .await?;
    let Some(creneau) = creneau else {
        return Err(BesoinPosteError::CreneauNotFound);
    };
    if !creneau_in_commission_scope(&creneau, commission_id, propre) {
        return Err(BesoinPosteError::CreneauHorsPerimetre);
    }

    let besoin_commission: Option<i32> = sqlx::query_scalar(
        "SELECT nb_requis FROM besoin_commission \
         WHERE creneau_applicatif_id = $1 AND commission_id = $2",
    )
    .bind(input.creneau_applicatif_id)
    .bind(commission_id)
    .fetch_optional(pool)
    .await?;
    let besoin_commission = besoin_commission.unwrap_or(0);

    let autres_postes_total: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(nb_requis), 0) FROM besoin_poste \
         WHERE creneau_applicatif_id = $1 AND poste_id != $2",
    )
    .bind(input.creneau_applicatif_id)
    .bind(input.poste_id)
    .fetch_one(pool)
    .await?;

    let nouveau_total = autres_postes_total as i32 + input.nb_requis;
    if nouveau_total > besoin_commission {
        return Err(BesoinPosteError::DepasseBesoinCommission(
            nouveau_total,
            besoin_commission,
        ));
    }

    sqlx::query(
        "INSERT INTO besoin_poste \
            (evenement_id, creneau_applicatif_id, commission_id, poste_id, nb_requis, \
             created_by, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $6) \
         ON CONFLICT (creneau_applicatif_id, poste_id) \
         DO UPDATE SET nb_requis = $5, updated_by = $6, updated_at = now()",
    )
    .bind(evenement_id)
    .bind(input.creneau_applicatif_id)
    .bind(commission_id)
    .bind(input.poste_id)
    .bind(input.nb_requis)
    .bind(admin_id)
    .execute(pool)
    .await?;

    Ok(())
}
