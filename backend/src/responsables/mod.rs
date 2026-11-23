//! Responsables de commission, par événement (specs.md §5.4, wireframe s9).
//! Être responsable d'une commission occupe le bénévole sur toute la grille
//! de cette commission pour tout l'événement ; cumuler plusieurs
//! responsabilités est permis sauf si leurs grilles se chevauchent dans le
//! temps sans que le bénévole soit responsable des deux. Pas d'écran dédié
//! pour les responsables de poste (§5.5) — purement informatif, désigné
//! uniquement depuis l'écran Planning (hors scope, pas encore construit).

pub mod endpoint;

use chrono::{NaiveDate, NaiveTime};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AddResponsableError {
    #[error("commission introuvable")]
    CommissionNotFound,
    #[error("bénévole introuvable")]
    BenevoleNotFound,
    #[error("ce bénévole n'a pas soumis de disponibilités pour cet événement")]
    BenevoleHorsEvenement,
    #[error("ce bénévole est déjà responsable de cette commission")]
    DejaResponsable,
    #[error(
        "{0} est déjà responsable de \"{1}\", dont au moins un créneau chevauche cette commission"
    )]
    ChevauchementAvecAutreCommission(String, String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveResponsableError {
    #[error("ce bénévole n'est pas responsable de cette commission")]
    NotFound,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(sqlx::FromRow)]
struct CommissionGridRow {
    id: Uuid,
    nom: String,
    grille_creneaux: String,
}

#[derive(Clone, Copy, PartialEq)]
struct Interval {
    jour: NaiveDate,
    heure_debut: NaiveTime,
    heure_fin: NaiveTime,
}

fn intervals_overlap(a: &[Interval], b: &[Interval]) -> bool {
    a.iter().any(|x| {
        b.iter()
            .any(|y| x.jour == y.jour && x.heure_debut < y.heure_fin && x.heure_fin > y.heure_debut)
    })
}

async fn commission_grid(
    pool: &sqlx::PgPool,
    evenement_id: Uuid,
    commission: &CommissionGridRow,
) -> Result<Vec<Interval>, sqlx::Error> {
    let scope = (commission.grille_creneaux == "propre").then_some(commission.id);
    sqlx::query_as(
        "SELECT jour, heure_debut, heure_fin FROM creneau_applicatif \
         WHERE evenement_id = $1 AND commission_id IS NOT DISTINCT FROM $2",
    )
    .bind(evenement_id)
    .bind(scope)
    .fetch_all(pool)
    .await
    .map(|rows: Vec<(NaiveDate, NaiveTime, NaiveTime)>| {
        rows.into_iter()
            .map(|(jour, heure_debut, heure_fin)| Interval {
                jour,
                heure_debut,
                heure_fin,
            })
            .collect()
    })
}

pub async fn add_responsable(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    commission_id: Uuid,
    soumission_id: Uuid,
) -> Result<(), AddResponsableError> {
    let commission: Option<CommissionGridRow> =
        sqlx::query_as("SELECT id, nom, grille_creneaux FROM commission WHERE id = $1")
            .bind(commission_id)
            .fetch_optional(pool)
            .await?;
    let Some(commission) = commission else {
        return Err(AddResponsableError::CommissionNotFound);
    };

    let soumission_evenement_id: Option<Uuid> =
        sqlx::query_scalar("SELECT evenement_id FROM soumission WHERE id = $1")
            .bind(soumission_id)
            .fetch_optional(pool)
            .await?;
    match soumission_evenement_id {
        None => return Err(AddResponsableError::BenevoleNotFound),
        Some(id) if id != evenement_id => return Err(AddResponsableError::BenevoleHorsEvenement),
        _ => {}
    }

    let target_grid = commission_grid(pool, evenement_id, &commission).await?;

    let autres: Vec<CommissionGridRow> = sqlx::query_as(
        "SELECT co.id, co.nom, co.grille_creneaux FROM responsable_commission rc \
         JOIN commission co ON co.id = rc.commission_id \
         WHERE rc.evenement_id = $1 AND rc.soumission_id = $2 AND rc.commission_id != $3",
    )
    .bind(evenement_id)
    .bind(soumission_id)
    .bind(commission_id)
    .fetch_all(pool)
    .await?;

    for autre in &autres {
        let autre_grid = commission_grid(pool, evenement_id, autre).await?;
        if intervals_overlap(&target_grid, &autre_grid) {
            let nom: String =
                sqlx::query_scalar("SELECT nom || ' ' || prenom FROM soumission WHERE id = $1")
                    .bind(soumission_id)
                    .fetch_one(pool)
                    .await?;
            return Err(AddResponsableError::ChevauchementAvecAutreCommission(
                nom,
                autre.nom.clone(),
            ));
        }
    }

    sqlx::query(
        "INSERT INTO responsable_commission (evenement_id, commission_id, soumission_id, created_by) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(evenement_id)
    .bind(commission_id)
    .bind(soumission_id)
    .bind(admin_id)
    .execute(pool)
    .await
    .map_err(|e| {
        if matches!(&e, sqlx::Error::Database(db_err) if db_err.is_unique_violation()) {
            AddResponsableError::DejaResponsable
        } else {
            AddResponsableError::Database(e)
        }
    })?;

    Ok(())
}

pub async fn remove_responsable(
    pool: &sqlx::PgPool,
    evenement_id: Uuid,
    commission_id: Uuid,
    soumission_id: Uuid,
) -> Result<(), RemoveResponsableError> {
    let rows_affected = sqlx::query(
        "DELETE FROM responsable_commission \
         WHERE evenement_id = $1 AND commission_id = $2 AND soumission_id = $3",
    )
    .bind(evenement_id)
    .bind(commission_id)
    .bind(soumission_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(RemoveResponsableError::NotFound);
    }

    Ok(())
}
