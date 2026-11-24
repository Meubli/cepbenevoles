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

#[derive(Debug, thiserror::Error)]
pub enum DupliquerJourError {
    #[error("le jour source et le jour cible doivent être différents")]
    SameJour,
    #[error("ce jour est hors des dates de l'événement")]
    JourHorsEvenement,
    #[error("événement introuvable")]
    EvenementNotFound,
    #[error("le jour source n'a aucun créneau de grille générale sur cet événement")]
    SourceEmpty,
    #[error("le jour cible a déjà des créneaux de grille générale sur cet événement")]
    TargetNotEmpty,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Duplicates `source_jour`'s grille générale créneaux (§4.1) to
/// `target_jour`, within the same événement, as a time-saving shortcut when
/// several days share the same schedule — along with any créneaux repas
/// (§4.4) and form mapping entries (§4.1 last point) defined on
/// `source_jour`, rebuilt on `target_jour` against the newly copied
/// créneaux (same `nom`/`slot_form`). Grilles propres (§4.3) are untouched
/// — they already have their own commission-to-commission duplication.
pub async fn dupliquer_jour(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    source_jour: NaiveDate,
    target_jour: NaiveDate,
) -> Result<(), DupliquerJourError> {
    if source_jour == target_jour {
        return Err(DupliquerJourError::SameJour);
    }
    let bounds = evenement_dates(pool, evenement_id)
        .await
        .map_err(|_| DupliquerJourError::EvenementNotFound)?;
    if target_jour < bounds.0 || target_jour > bounds.1 {
        return Err(DupliquerJourError::JourHorsEvenement);
    }

    let mut tx = pool.begin().await?;

    let target_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM creneau_applicatif \
         WHERE evenement_id = $1 AND commission_id IS NULL AND jour = $2",
    )
    .bind(evenement_id)
    .bind(target_jour)
    .fetch_one(&mut *tx)
    .await?;
    if target_count > 0 {
        return Err(DupliquerJourError::TargetNotEmpty);
    }

    struct SourceCreneau {
        id: Uuid,
        heure_debut: NaiveTime,
        heure_fin: NaiveTime,
        ordre: i32,
    }
    let sources: Vec<SourceCreneau> = sqlx::query_as::<_, (Uuid, NaiveTime, NaiveTime, i32)>(
        "SELECT id, heure_debut, heure_fin, ordre FROM creneau_applicatif \
         WHERE evenement_id = $1 AND commission_id IS NULL AND jour = $2",
    )
    .bind(evenement_id)
    .bind(source_jour)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|(id, heure_debut, heure_fin, ordre)| SourceCreneau {
        id,
        heure_debut,
        heure_fin,
        ordre,
    })
    .collect();
    if sources.is_empty() {
        return Err(DupliquerJourError::SourceEmpty);
    }

    let mut id_map = std::collections::HashMap::with_capacity(sources.len());
    for c in &sources {
        let new_id: Uuid = sqlx::query_scalar(
            "INSERT INTO creneau_applicatif \
                (evenement_id, commission_id, jour, heure_debut, heure_fin, ordre, \
                 created_by, updated_by) \
             VALUES ($1, NULL, $2, $3, $4, $5, $6, $6) RETURNING id",
        )
        .bind(evenement_id)
        .bind(target_jour)
        .bind(c.heure_debut)
        .bind(c.heure_fin)
        .bind(c.ordre)
        .bind(admin_id)
        .fetch_one(&mut *tx)
        .await?;
        id_map.insert(c.id, new_id);
    }

    let repas: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, nom FROM creneau_repas WHERE evenement_id = $1 AND jour = $2")
            .bind(evenement_id)
            .bind(source_jour)
            .fetch_all(&mut *tx)
            .await?;
    for (repas_id, nom) in repas {
        let membres: Vec<Uuid> = sqlx::query_scalar(
            "SELECT creneau_applicatif_id FROM creneau_repas_membre WHERE creneau_repas_id = $1",
        )
        .bind(repas_id)
        .fetch_all(&mut *tx)
        .await?;

        let new_repas_id: Uuid = sqlx::query_scalar(
            "INSERT INTO creneau_repas (evenement_id, jour, nom, created_by, updated_by) \
             VALUES ($1, $2, $3, $4, $4) RETURNING id",
        )
        .bind(evenement_id)
        .bind(target_jour)
        .bind(&nom)
        .bind(admin_id)
        .fetch_one(&mut *tx)
        .await?;

        for old_membre_id in membres {
            if let Some(new_membre_id) = id_map.get(&old_membre_id) {
                sqlx::query(
                    "INSERT INTO creneau_repas_membre (creneau_repas_id, creneau_applicatif_id) \
                     VALUES ($1, $2)",
                )
                .bind(new_repas_id)
                .bind(new_membre_id)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    let mapping: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT slot_form, creneau_applicatif_id FROM creneau_form_mapping \
         WHERE evenement_id = $1 AND jour = $2",
    )
    .bind(evenement_id)
    .bind(source_jour)
    .fetch_all(&mut *tx)
    .await?;
    for (slot_form, old_creneau_id) in mapping {
        if let Some(new_creneau_id) = id_map.get(&old_creneau_id) {
            sqlx::query(
                "INSERT INTO creneau_form_mapping \
                    (evenement_id, jour, slot_form, creneau_applicatif_id) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(evenement_id)
            .bind(target_jour)
            .bind(&slot_form)
            .bind(new_creneau_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
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

/// Minimum de créneaux applicatifs par créneau repas (§4.4) — avec un seul
/// créneau, un bénévole ne serait jamais affectable sur ce créneau
/// (contrainte toujours violée), donc probablement une erreur de saisie.
pub const CRENEAU_REPAS_MIN_MEMBRES: usize = 2;

pub struct CreneauRepasInput {
    pub jour: NaiveDate,
    pub nom: String,
    pub creneau_ids: Vec<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreneauRepasError {
    #[error("le nom ne peut pas être vide")]
    EmptyNom,
    #[error("un créneau repas doit avoir au moins {CRENEAU_REPAS_MIN_MEMBRES} créneaux")]
    NotEnoughMembres,
    #[error("ce jour est hors des dates de l'événement")]
    JourHorsEvenement,
    #[error("événement introuvable")]
    EvenementNotFound,
    #[error("ce créneau n'existe pas sur cet événement, n'est pas dans la grille générale, ou n'est pas ce jour-là")]
    InvalidCreneau,
    #[error("créneau repas introuvable")]
    NotFound,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

fn dedup_ids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

async fn validate_creneau_repas_input(
    pool: &sqlx::PgPool,
    tx: &mut sqlx::PgConnection,
    evenement_id: Uuid,
    input: &CreneauRepasInput,
    ids: &[Uuid],
) -> Result<(), CreneauRepasError> {
    if input.nom.trim().is_empty() {
        return Err(CreneauRepasError::EmptyNom);
    }
    if ids.len() < CRENEAU_REPAS_MIN_MEMBRES {
        return Err(CreneauRepasError::NotEnoughMembres);
    }
    let bounds = evenement_dates(pool, evenement_id)
        .await
        .map_err(|_| CreneauRepasError::EvenementNotFound)?;
    if input.jour < bounds.0 || input.jour > bounds.1 {
        return Err(CreneauRepasError::JourHorsEvenement);
    }
    for id in ids {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM creneau_applicatif \
             WHERE id = $1 AND evenement_id = $2 AND commission_id IS NULL AND jour = $3)",
        )
        .bind(id)
        .bind(evenement_id)
        .bind(input.jour)
        .fetch_one(&mut *tx)
        .await?;
        if !valid {
            return Err(CreneauRepasError::InvalidCreneau);
        }
    }
    Ok(())
}

pub async fn create_creneau_repas(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    input: CreneauRepasInput,
) -> Result<Uuid, CreneauRepasError> {
    let ids = dedup_ids(input.creneau_ids.clone());
    let mut tx = pool.begin().await?;
    validate_creneau_repas_input(pool, &mut tx, evenement_id, &input, &ids).await?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO creneau_repas (evenement_id, jour, nom, created_by, updated_by) \
         VALUES ($1, $2, $3, $4, $4) RETURNING id",
    )
    .bind(evenement_id)
    .bind(input.jour)
    .bind(input.nom.trim())
    .bind(admin_id)
    .fetch_one(&mut *tx)
    .await?;

    for creneau_id in &ids {
        sqlx::query(
            "INSERT INTO creneau_repas_membre (creneau_repas_id, creneau_applicatif_id) \
             VALUES ($1, $2)",
        )
        .bind(id)
        .bind(creneau_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(id)
}

pub async fn update_creneau_repas(
    pool: &sqlx::PgPool,
    admin_id: Uuid,
    evenement_id: Uuid,
    creneau_repas_id: Uuid,
    input: CreneauRepasInput,
) -> Result<(), CreneauRepasError> {
    let ids = dedup_ids(input.creneau_ids.clone());
    let mut tx = pool.begin().await?;
    validate_creneau_repas_input(pool, &mut tx, evenement_id, &input, &ids).await?;

    let rows_affected = sqlx::query(
        "UPDATE creneau_repas SET nom = $1, jour = $2, updated_by = $3, updated_at = now() \
         WHERE id = $4 AND evenement_id = $5",
    )
    .bind(input.nom.trim())
    .bind(input.jour)
    .bind(admin_id)
    .bind(creneau_repas_id)
    .bind(evenement_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if rows_affected == 0 {
        return Err(CreneauRepasError::NotFound);
    }

    sqlx::query("DELETE FROM creneau_repas_membre WHERE creneau_repas_id = $1")
        .bind(creneau_repas_id)
        .execute(&mut *tx)
        .await?;
    for creneau_id in &ids {
        sqlx::query(
            "INSERT INTO creneau_repas_membre (creneau_repas_id, creneau_applicatif_id) \
             VALUES ($1, $2)",
        )
        .bind(creneau_repas_id)
        .bind(creneau_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn delete_creneau_repas(
    pool: &sqlx::PgPool,
    evenement_id: Uuid,
    creneau_repas_id: Uuid,
) -> Result<(), CreneauRepasError> {
    let rows_affected =
        sqlx::query("DELETE FROM creneau_repas WHERE id = $1 AND evenement_id = $2")
            .bind(creneau_repas_id)
            .bind(evenement_id)
            .execute(pool)
            .await?
            .rows_affected();
    if rows_affected == 0 {
        return Err(CreneauRepasError::NotFound);
    }
    Ok(())
}
