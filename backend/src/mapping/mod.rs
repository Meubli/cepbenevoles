//! Resolves raw wish labels (`soumission_libelle`, specs.md §3.2) into
//! `SouhaitCommission` rows or the `Soumission.polyvalent` flag, once an
//! admin has mapped the label. Also serves the Mapping form screen (listing
//! + save) and the Commissions picker it needs.

pub mod endpoint;

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingKind {
    Commission,
    Polyvalent,
}

impl MappingKind {
    fn as_db_str(self) -> &'static str {
        match self {
            MappingKind::Commission => "commission",
            MappingKind::Polyvalent => "polyvalent",
        }
    }
}

/// Resolves every not-yet-resolved `soumission_libelle` whose label now has
/// a mapping — either into a `souhait_commission` row or, for the
/// polyvalent label, by setting `soumission.polyvalent`. Already-resolved
/// rows are never touched (specs.md §3.2: changing a mapping later must not
/// rewrite wishes already resolved under the old one).
///
/// Idempotent and safe to call repeatedly: after every mapping save, and
/// after every sync (so a submission inserted mid-mapping-edit still gets
/// resolved on the next sweep instead of staying stuck).
pub async fn resolve_pending_libelles(pool: &PgPool) -> Result<i64, sqlx::Error> {
    // A single statement, not three sequential ones: under READ COMMITTED,
    // separate statements each take their own snapshot, so a sync or
    // another mapping save landing a new soumission_libelle row between
    // them could get marked "resolved" by the last statement without ever
    // having had its souhait_commission row created — a silently lost wish.
    // Every CTE here runs against the same snapshot and, because `ins` and
    // `poly` read from `pending`'s RETURNING rather than re-querying the
    // tables, a concurrent sweep can only see rows this one hasn't claimed
    // yet (row locks from the first UPDATE make the second sweep wait, then
    // its own `resolved_at IS NULL` check finds nothing left to do).
    let resolved_count: i64 = sqlx::query_scalar(
        "WITH pending AS ( \
             UPDATE soumission_libelle sl SET resolved_at = now() \
             FROM mapping_souhait_form m \
             WHERE sl.resolved_at IS NULL AND m.libelle = sl.libelle_brut \
             RETURNING sl.soumission_id, m.kind, m.commission_id, m.poste_id \
         ), ins AS ( \
             INSERT INTO souhait_commission (soumission_id, commission_id, poste_id) \
             SELECT soumission_id, commission_id, poste_id FROM pending WHERE kind = 'commission' \
             ON CONFLICT DO NOTHING \
             RETURNING 1 \
         ), poly AS ( \
             UPDATE soumission SET polyvalent = true \
             WHERE id IN (SELECT soumission_id FROM pending WHERE kind = 'polyvalent') \
             RETURNING 1 \
         ) \
         SELECT COUNT(*) FROM pending",
    )
    .fetch_one(pool)
    .await?;

    Ok(resolved_count)
}

#[derive(Debug, thiserror::Error)]
pub enum SaveMappingError {
    #[error("libellé inconnu : aucune soumission ne l'a utilisé")]
    UnknownLabel,
    #[error("commission introuvable ou inactive")]
    InvalidCommission,
    #[error("ce poste n'appartient pas à la commission choisie")]
    PosteNotInCommission,
    #[error("cette commission n'a pas de poste, retirez le poste choisi")]
    CommissionHasNoPostes,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct SaveMapping {
    pub libelle: String,
    pub kind: MappingKind,
    pub commission_id: Option<Uuid>,
    pub poste_id: Option<Uuid>,
}

/// Creates or updates one label's mapping, validating the target, then
/// immediately runs the resolution sweep so pending wishes using this label
/// are resolved right away (specs.md §3.2's "rétroactif").
pub async fn save_mapping(
    pool: &PgPool,
    admin_id: Uuid,
    input: SaveMapping,
) -> Result<(), SaveMappingError> {
    let label_seen: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM soumission_libelle WHERE libelle_brut = $1)",
    )
    .bind(&input.libelle)
    .fetch_one(pool)
    .await?;
    if !label_seen {
        return Err(SaveMappingError::UnknownLabel);
    }

    if input.kind == MappingKind::Commission {
        let commission_id = input
            .commission_id
            .ok_or(SaveMappingError::InvalidCommission)?;

        let a_des_postes: Option<bool> = sqlx::query_scalar(
            "SELECT a_des_postes FROM commission WHERE id = $1 AND actif = true",
        )
        .bind(commission_id)
        .fetch_optional(pool)
        .await?;
        let a_des_postes = a_des_postes.ok_or(SaveMappingError::InvalidCommission)?;

        if let Some(poste_id) = input.poste_id {
            if !a_des_postes {
                return Err(SaveMappingError::CommissionHasNoPostes);
            }
            let belongs: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM poste WHERE id = $1 AND commission_id = $2)",
            )
            .bind(poste_id)
            .bind(commission_id)
            .fetch_one(pool)
            .await?;
            if !belongs {
                return Err(SaveMappingError::PosteNotInCommission);
            }
        }
    }

    sqlx::query(
        "INSERT INTO mapping_souhait_form (libelle, kind, commission_id, poste_id, created_by, updated_by) \
         VALUES ($1, $2, $3, $4, $5, $5) \
         ON CONFLICT (libelle) DO UPDATE SET \
            kind = EXCLUDED.kind, \
            commission_id = EXCLUDED.commission_id, \
            poste_id = EXCLUDED.poste_id, \
            updated_by = EXCLUDED.updated_by, \
            updated_at = now()",
    )
    .bind(&input.libelle)
    .bind(input.kind.as_db_str())
    .bind(input.commission_id)
    .bind(input.poste_id)
    .bind(admin_id)
    .execute(pool)
    .await?;

    resolve_pending_libelles(pool).await?;

    Ok(())
}
