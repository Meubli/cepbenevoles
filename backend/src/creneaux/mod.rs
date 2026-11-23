//! Écran Créneaux (specs.md §4.1, §4.2, §4.3): gabarit de créneaux
//! applicatifs par jour/événement (grille générale), grilles propres par
//! commission (+ duplication d'une grille existante comme point de départ),
//! bornes matin/midi/soir par événement, et mapping créneau form -> créneaux
//! applicatifs (§4.1 dernier point). Périmètre choisi avec l'utilisateur —
//! les règles d'enchaînement elles-mêmes (§4.2 points 1-3) ne sont pas
//! implémentées ici, seulement leur paramétrage.

pub mod endpoint;

use chrono::{NaiveDate, NaiveTime};
use uuid::Uuid;

pub struct CreneauInput {
    pub commission_id: Option<Uuid>,
    pub jour: NaiveDate,
    pub heure_debut: NaiveTime,
    pub heure_fin: NaiveTime,
    pub ordre: i32,
}

pub struct BornesInput {
    pub fin_matin: NaiveTime,
    pub fin_midi: NaiveTime,
    pub fin_aprem: NaiveTime,
}

#[derive(Debug, thiserror::Error)]
pub enum CreneauError {
    #[error("heure_fin doit être postérieure à heure_debut")]
    InvalidHeures,
    #[error("ce jour est hors des dates de l'événement")]
    JourHorsEvenement,
    #[error("événement introuvable")]
    EvenementNotFound,
    #[error("commission introuvable")]
    CommissionNotFound,
    #[error("cette commission n'utilise pas une grille propre")]
    CommissionNotPropre,
    #[error("créneau introuvable")]
    NotFound,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

async fn evenement_dates(
    pool: &sqlx::PgPool,
    evenement_id: Uuid,
) -> Result<(NaiveDate, NaiveDate), CreneauError> {
    let row: Option<(NaiveDate, NaiveDate)> =
        sqlx::query_as("SELECT date_debut, date_fin FROM evenement WHERE id = $1")
            .bind(evenement_id)
            .fetch_optional(pool)
            .await?;
    row.ok_or(CreneauError::EvenementNotFound)
}

/// A commission used as a `creneau_applicatif.commission_id` must exist and
/// use a grille propre (§4.3) — otherwise its créneaux would never be read
/// by anything (the générale grid is the one with `commission_id IS NULL`).
async fn check_commission_propre(
    pool: &sqlx::PgPool,
    commission_id: Uuid,
) -> Result<(), CreneauError> {
    let grille: Option<String> =
        sqlx::query_scalar("SELECT grille_creneaux FROM commission WHERE id = $1")
            .bind(commission_id)
            .fetch_optional(pool)
            .await?;
    match grille {
        None => Err(CreneauError::CommissionNotFound),
        Some(g) if g != "propre" => Err(CreneauError::CommissionNotPropre),
        Some(_) => Ok(()),
    }
}

fn validate_input(
    input: &CreneauInput,
    bounds: (NaiveDate, NaiveDate),
) -> Result<(), CreneauError> {
    if input.heure_fin <= input.heure_debut {
        return Err(CreneauError::InvalidHeures);
    }
    if input.jour < bounds.0 || input.jour > bounds.1 {
        return Err(CreneauError::JourHorsEvenement);
    }
    Ok(())
}

pub async fn create_creneau(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    input: CreneauInput,
) -> Result<Uuid, CreneauError> {
    let bounds = evenement_dates(pool, evenement_id).await?;
    validate_input(&input, bounds)?;
    if let Some(commission_id) = input.commission_id {
        check_commission_propre(pool, commission_id).await?;
    }

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO creneau_applicatif \
            (evenement_id, commission_id, jour, heure_debut, heure_fin, ordre, \
             created_by, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7) \
         RETURNING id",
    )
    .bind(evenement_id)
    .bind(input.commission_id)
    .bind(input.jour)
    .bind(input.heure_debut)
    .bind(input.heure_fin)
    .bind(input.ordre)
    .bind(admin_id)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn update_creneau(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    creneau_id: Uuid,
    input: CreneauInput,
) -> Result<(), CreneauError> {
    let bounds = evenement_dates(pool, evenement_id).await?;
    validate_input(&input, bounds)?;
    if let Some(commission_id) = input.commission_id {
        check_commission_propre(pool, commission_id).await?;
    }

    let rows_affected = sqlx::query(
        "UPDATE creneau_applicatif SET \
            commission_id = $1, jour = $2, heure_debut = $3, heure_fin = $4, ordre = $5, \
            updated_by = $6, updated_at = now() \
         WHERE id = $7 AND evenement_id = $8",
    )
    .bind(input.commission_id)
    .bind(input.jour)
    .bind(input.heure_debut)
    .bind(input.heure_fin)
    .bind(input.ordre)
    .bind(admin_id)
    .bind(creneau_id)
    .bind(evenement_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(CreneauError::NotFound);
    }
    Ok(())
}

pub async fn delete_creneau(
    pool: &sqlx::PgPool,
    evenement_id: Uuid,
    creneau_id: Uuid,
) -> Result<(), CreneauError> {
    let rows_affected =
        sqlx::query("DELETE FROM creneau_applicatif WHERE id = $1 AND evenement_id = $2")
            .bind(creneau_id)
            .bind(evenement_id)
            .execute(pool)
            .await?
            .rows_affected();

    if rows_affected == 0 {
        return Err(CreneauError::NotFound);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum BornesError {
    #[error("les bornes doivent être strictement croissantes (matin < midi < aprem)")]
    NotIncreasing,
    #[error("événement introuvable")]
    NotFound,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn update_bornes(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    input: BornesInput,
) -> Result<(), BornesError> {
    if !(input.fin_matin < input.fin_midi && input.fin_midi < input.fin_aprem) {
        return Err(BornesError::NotIncreasing);
    }

    let rows_affected = sqlx::query(
        "UPDATE evenement SET \
            born_fin_matin = $1, born_fin_midi = $2, born_fin_aprem = $3, \
            updated_by = $4, updated_at = now() \
         WHERE id = $5",
    )
    .bind(input.fin_matin)
    .bind(input.fin_midi)
    .bind(input.fin_aprem)
    .bind(admin_id)
    .bind(evenement_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(BornesError::NotFound);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum DupliquerError {
    #[error("commission source introuvable")]
    SourceNotFound,
    #[error("commission cible introuvable")]
    TargetNotFound,
    #[error("la commission cible n'utilise pas une grille propre")]
    TargetNotPropre,
    #[error("la commission source n'a aucun créneau sur cet événement")]
    SourceEmpty,
    #[error("la commission cible a déjà des créneaux sur cet événement")]
    TargetNotEmpty,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Duplicates `source_commission_id`'s créneaux on `evenement_id` as a
/// starting point for `target_commission_id`'s own grille propre (§4.3) —
/// a one-shot copy, no structural link kept between the two afterwards.
pub async fn dupliquer_grille(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    source_commission_id: Uuid,
    target_commission_id: Uuid,
) -> Result<(), DupliquerError> {
    let source_grille: Option<String> =
        sqlx::query_scalar("SELECT grille_creneaux FROM commission WHERE id = $1")
            .bind(source_commission_id)
            .fetch_optional(pool)
            .await?;
    if source_grille.is_none() {
        return Err(DupliquerError::SourceNotFound);
    }

    let target_grille: Option<String> =
        sqlx::query_scalar("SELECT grille_creneaux FROM commission WHERE id = $1")
            .bind(target_commission_id)
            .fetch_optional(pool)
            .await?;
    match target_grille {
        None => return Err(DupliquerError::TargetNotFound),
        Some(g) if g != "propre" => return Err(DupliquerError::TargetNotPropre),
        Some(_) => {}
    }

    let target_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM creneau_applicatif WHERE evenement_id = $1 AND commission_id = $2",
    )
    .bind(evenement_id)
    .bind(target_commission_id)
    .fetch_one(pool)
    .await?;
    if target_count > 0 {
        return Err(DupliquerError::TargetNotEmpty);
    }

    let rows_affected = sqlx::query(
        "INSERT INTO creneau_applicatif \
            (evenement_id, commission_id, jour, heure_debut, heure_fin, ordre, \
             created_by, updated_by) \
         SELECT evenement_id, $3, jour, heure_debut, heure_fin, ordre, $2, $2 \
         FROM creneau_applicatif \
         WHERE evenement_id = $1 AND commission_id = $4",
    )
    .bind(evenement_id)
    .bind(admin_id)
    .bind(target_commission_id)
    .bind(source_commission_id)
    .execute(pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(DupliquerError::SourceEmpty);
    }
    Ok(())
}

pub struct MappingEntryInput {
    pub jour: NaiveDate,
    pub slot_form: String,
    pub creneau_applicatif_id: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    #[error("slot_form invalide — attendu matin/midi/aprem/soir")]
    InvalidSlot,
    #[error("le créneau {0} n'existe pas sur cet événement, n'est pas dans la grille générale, ou n'est pas ce jour-là")]
    InvalidCreneau(Uuid),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

const VALID_SLOTS: [&str; 4] = ["matin", "midi", "aprem", "soir"];

/// Replaces the whole form-slot -> créneaux-applicatifs mapping for one
/// événement (§4.1 last point). Each `creneau_applicatif_id` must belong to
/// this événement's grille générale (`commission_id IS NULL`) and match the
/// entry's `jour` — the form has no notion of commission, so a grille
/// propre créneau could never be reached by a submission's declared
/// availability anyway.
pub async fn replace_mapping(
    pool: &sqlx::PgPool,
    evenement_id: Uuid,
    entries: Vec<MappingEntryInput>,
) -> Result<(), MappingError> {
    for entry in &entries {
        if !VALID_SLOTS.contains(&entry.slot_form.as_str()) {
            return Err(MappingError::InvalidSlot);
        }
    }

    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM creneau_form_mapping WHERE evenement_id = $1")
        .bind(evenement_id)
        .execute(&mut *tx)
        .await?;

    for entry in entries {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM creneau_applicatif \
             WHERE id = $1 AND evenement_id = $2 AND commission_id IS NULL AND jour = $3)",
        )
        .bind(entry.creneau_applicatif_id)
        .bind(evenement_id)
        .bind(entry.jour)
        .fetch_one(&mut *tx)
        .await?;
        if !valid {
            return Err(MappingError::InvalidCreneau(entry.creneau_applicatif_id));
        }

        sqlx::query(
            "INSERT INTO creneau_form_mapping (evenement_id, jour, slot_form, creneau_applicatif_id) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(evenement_id)
        .bind(entry.jour)
        .bind(&entry.slot_form)
        .bind(entry.creneau_applicatif_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
