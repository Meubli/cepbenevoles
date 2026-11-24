//! Écran Bénévoles — saisie manuelle d'un bénévole hors Google Form
//! (specs.md §3.4, wireframe s7). Crée une `soumission` par événement
//! coché, comme si elle venait du form : mêmes tables, mêmes règles de
//! doublon (§9, clé `(evenement_id, date_soumission, email)`), mais sans
//! `soumission_libelle` ni `row_source_id` — les souhaits de commission
//! sont choisis directement dans le référentiel (commission/poste), pas
//! saisis comme libellé brut à mapper ensuite.

pub mod endpoint;

use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

/// Même limite que `Commission.nom`/`Admin.nom` — casse-layout, pas une
/// vraie contrainte métier.
pub const NOM_MAX_LEN: usize = 100;
pub const EMAIL_MAX_LEN: usize = 255;
pub const TELEPHONE_MAX_LEN: usize = 30;
pub const TAILLE_TSHIRT_MAX_LEN: usize = 20;

const KNOWN_SLOTS: [&str; 4] = ["matin", "midi", "aprem", "soir"];

/// Labels exacts stockés en base pour `tranche_age` — doit rester en phase
/// avec `crate::sync::TrancheAge::label()`, seule source pour les
/// soumissions importées du form réel.
const KNOWN_TRANCHES_AGE: [&str; 4] = ["10 - 12 ans", "13 - 14 ans", "15 - 17 ans", "18 ans et +"];

pub struct SouhaitInput {
    pub commission_id: Uuid,
    pub poste_id: Option<Uuid>,
}

pub struct SoumissionInput {
    pub evenement_id: Uuid,
    pub disponibilites: Vec<(NaiveDate, String)>,
    pub polyvalent: bool,
    pub souhaits: Vec<SouhaitInput>,
}

pub struct NouveauBenevoleInput {
    pub nom: String,
    pub prenom: String,
    pub email: String,
    pub telephone: String,
    pub tranche_age: String,
    pub permis_b: bool,
    pub taille_tshirt: String,
    pub soumissions: Vec<SoumissionInput>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateBenevoleError {
    #[error("le nom ne peut pas être vide")]
    EmptyNom,
    #[error("le nom ne peut pas dépasser {NOM_MAX_LEN} caractères")]
    NomTooLong,
    #[error("le prénom ne peut pas être vide")]
    EmptyPrenom,
    #[error("le prénom ne peut pas dépasser {NOM_MAX_LEN} caractères")]
    PrenomTooLong,
    #[error("adresse email invalide")]
    InvalidEmail,
    #[error("l'email ne peut pas dépasser {EMAIL_MAX_LEN} caractères")]
    EmailTooLong,
    #[error("le téléphone ne peut pas être vide")]
    EmptyTelephone,
    #[error("le téléphone ne peut pas dépasser {TELEPHONE_MAX_LEN} caractères")]
    TelephoneTooLong,
    #[error("numéro de téléphone invalide")]
    InvalidTelephone,
    #[error("tranche d'âge invalide")]
    InvalidTrancheAge,
    #[error("la taille de tee-shirt ne peut pas être vide")]
    EmptyTailleTshirt,
    #[error("la taille de tee-shirt ne peut pas dépasser {TAILLE_TSHIRT_MAX_LEN} caractères")]
    TailleTshirtTooLong,
    #[error("sélectionnez au moins un événement")]
    NoEvenement,
    #[error("un même événement ne peut pas être coché deux fois")]
    DuplicateEvenement,
    #[error("événement introuvable")]
    EvenementNotFound,
    #[error("ce jour est hors des dates de l'événement")]
    JourHorsEvenement,
    #[error("créneau de disponibilité invalide")]
    InvalidSlot,
    #[error("commission introuvable")]
    CommissionNotFound,
    #[error("ce poste n'appartient pas à la commission choisie")]
    PosteNotInCommission,
    #[error(
        "indiquez au moins un créneau de disponibilité ou un poste souhaité pour chaque événement"
    )]
    EmptySoumission,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Accepte les séparateurs usuels (espace, point, tiret, parenthèses) et un
/// `+` international en tête, mais exige au moins 10 chiffres pour rejeter
/// un numéro incohérent (ex: "1234") — cf. `is_valid_email` pour la même
/// logique côté email.
fn is_valid_telephone(telephone: &str) -> bool {
    let t = telephone.trim();
    if t.is_empty() {
        return false;
    }
    let rest = t.strip_prefix('+').unwrap_or(t);
    if rest.contains('+') {
        return false;
    }
    let digit_count = rest.chars().filter(|c| c.is_ascii_digit()).count();
    let only_allowed_chars = rest
        .chars()
        .all(|c| c.is_ascii_digit() || c == ' ' || c == '.' || c == '-' || c == '(' || c == ')');
    only_allowed_chars && digit_count >= 10
}

fn validate_identite(input: &NouveauBenevoleInput) -> Result<(), CreateBenevoleError> {
    if input.nom.trim().is_empty() {
        return Err(CreateBenevoleError::EmptyNom);
    }
    if input.nom.trim().chars().count() > NOM_MAX_LEN {
        return Err(CreateBenevoleError::NomTooLong);
    }
    if input.prenom.trim().is_empty() {
        return Err(CreateBenevoleError::EmptyPrenom);
    }
    if input.prenom.trim().chars().count() > NOM_MAX_LEN {
        return Err(CreateBenevoleError::PrenomTooLong);
    }
    if input.email.trim().chars().count() > EMAIL_MAX_LEN {
        return Err(CreateBenevoleError::EmailTooLong);
    }
    if !crate::admins::is_valid_email(input.email.trim()) {
        return Err(CreateBenevoleError::InvalidEmail);
    }
    if input.telephone.trim().is_empty() {
        return Err(CreateBenevoleError::EmptyTelephone);
    }
    if input.telephone.trim().chars().count() > TELEPHONE_MAX_LEN {
        return Err(CreateBenevoleError::TelephoneTooLong);
    }
    if !is_valid_telephone(&input.telephone) {
        return Err(CreateBenevoleError::InvalidTelephone);
    }
    if !KNOWN_TRANCHES_AGE.contains(&input.tranche_age.as_str()) {
        return Err(CreateBenevoleError::InvalidTrancheAge);
    }
    if input.taille_tshirt.trim().is_empty() {
        return Err(CreateBenevoleError::EmptyTailleTshirt);
    }
    if input.taille_tshirt.trim().chars().count() > TAILLE_TSHIRT_MAX_LEN {
        return Err(CreateBenevoleError::TailleTshirtTooLong);
    }
    if input.soumissions.is_empty() {
        return Err(CreateBenevoleError::NoEvenement);
    }
    let mut seen = std::collections::HashSet::new();
    for s in &input.soumissions {
        if !seen.insert(s.evenement_id) {
            return Err(CreateBenevoleError::DuplicateEvenement);
        }
        if s.disponibilites.is_empty() && !s.polyvalent && s.souhaits.is_empty() {
            return Err(CreateBenevoleError::EmptySoumission);
        }
    }
    Ok(())
}

/// Crée une `soumission` par événement coché, dans une seule transaction
/// (tout ou rien — une saisie multi-événements ratée à moitié serait
/// confuse pour l'admin). Retourne les ids créés, dans l'ordre d'entrée.
pub async fn create_benevole(
    pool: &PgPool,
    admin_id: Uuid,
    input: NouveauBenevoleInput,
) -> Result<Vec<Uuid>, CreateBenevoleError> {
    validate_identite(&input)?;
    let _ = admin_id; // pas de created_by sur soumission (même schéma que le sync)

    let mut tx = pool.begin().await?;
    let mut ids = Vec::with_capacity(input.soumissions.len());

    for soumission in &input.soumissions {
        let bounds: Option<(NaiveDate, NaiveDate)> =
            sqlx::query_as("SELECT date_debut, date_fin FROM evenement WHERE id = $1")
                .bind(soumission.evenement_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((date_debut, date_fin)) = bounds else {
            return Err(CreateBenevoleError::EvenementNotFound);
        };

        for (jour, slot) in &soumission.disponibilites {
            if *jour < date_debut || *jour > date_fin {
                return Err(CreateBenevoleError::JourHorsEvenement);
            }
            if !KNOWN_SLOTS.contains(&slot.as_str()) {
                return Err(CreateBenevoleError::InvalidSlot);
            }
        }

        for souhait in &soumission.souhaits {
            let a_des_postes: Option<bool> = sqlx::query_scalar(
                "SELECT a_des_postes FROM commission WHERE id = $1 AND actif = true",
            )
            .bind(souhait.commission_id)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(a_des_postes) = a_des_postes else {
                return Err(CreateBenevoleError::CommissionNotFound);
            };
            if let Some(poste_id) = souhait.poste_id {
                if !a_des_postes {
                    return Err(CreateBenevoleError::PosteNotInCommission);
                }
                let belongs: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM poste WHERE id = $1 AND commission_id = $2)",
                )
                .bind(poste_id)
                .bind(souhait.commission_id)
                .fetch_one(&mut *tx)
                .await?;
                if !belongs {
                    return Err(CreateBenevoleError::PosteNotInCommission);
                }
            }
        }

        let soumission_id: Uuid = sqlx::query_scalar(
            "INSERT INTO soumission \
                (evenement_id, nom, prenom, email, telephone, tranche_age, permis_b, \
                 taille_tshirt, date_soumission, polyvalent) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now(), $9) \
             RETURNING id",
        )
        .bind(soumission.evenement_id)
        .bind(input.nom.trim())
        .bind(input.prenom.trim())
        .bind(input.email.trim())
        .bind(input.telephone.trim())
        .bind(&input.tranche_age)
        .bind(input.permis_b)
        .bind(input.taille_tshirt.trim())
        .bind(soumission.polyvalent)
        .fetch_one(&mut *tx)
        .await?;

        for (jour, slot) in &soumission.disponibilites {
            sqlx::query(
                "INSERT INTO disponibilite_soumission (soumission_id, jour, slot) \
                 VALUES ($1, $2, $3)",
            )
            .bind(soumission_id)
            .bind(jour)
            .bind(slot)
            .execute(&mut *tx)
            .await?;
        }

        for souhait in &soumission.souhaits {
            sqlx::query(
                "INSERT INTO souhait_commission (soumission_id, commission_id, poste_id) \
                 VALUES ($1, $2, $3)",
            )
            .bind(soumission_id)
            .bind(souhait.commission_id)
            .bind(souhait.poste_id)
            .execute(&mut *tx)
            .await?;
        }

        ids.push(soumission_id);
    }

    tx.commit().await?;
    Ok(ids)
}
