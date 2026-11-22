//! Persists [`ParsedRow`]s into Postgres.
//!
//! Scope: submissions only (evenement, soumission, disponibilite_soumission,
//! soumission_libelle). No scheduling, no Sheets fetch orchestration, no
//! label->commission mapping (specs.md §3.2 — stays a separate concern) here.

use std::collections::HashMap;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{EventSlug, ParsedRow};

const KNOWN_SLOTS: [&str; 4] = ["matin", "midi", "aprem", "soir"];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    /// New `Soumission` rows created.
    pub inserted: usize,
    /// Rows that matched the `(evenement, date_soumission, email)` import
    /// key of an already-persisted submission — expected on a re-sync, not
    /// an error (specs.md §3.3).
    pub already_present: usize,
    /// Attendances for an `EventSlug` with no entry in `routing` — the
    /// caller hasn't configured which `Evenement` this form block feeds.
    pub unrouted: usize,
    /// Availability entries whose slot wasn't one of
    /// matin/midi/aprem/soir (a day-prefix stripping mismatch, cf.
    /// form_2027.rs) — skipped rather than failing the whole submission.
    pub skipped_availabilities: usize,
}

/// Inserts parsed rows, routing each attendance's [`EventSlug`] to the
/// `Evenement` id configured for it. One Sheet row can produce multiple
/// `Soumission`s (one per event attended) and is itself idempotent: running
/// this twice against the same parsed rows inserts nothing the second time.
pub async fn persist_parsed_rows(
    pool: &PgPool,
    routing: &HashMap<EventSlug, Uuid>,
    rows: &[ParsedRow],
) -> Result<SyncReport, sqlx::Error> {
    let mut report = SyncReport::default();
    let mut tx = pool.begin().await?;

    for row in rows {
        for attendance in &row.attendances {
            let Some(&evenement_id) = routing.get(&attendance.evenement) else {
                report.unrouted += 1;
                continue;
            };

            let soumission_id = insert_soumission(&mut tx, evenement_id, row).await?;
            let Some(soumission_id) = soumission_id else {
                report.already_present += 1;
                continue;
            };
            report.inserted += 1;

            for (date, slot) in &attendance.availabilities {
                if !KNOWN_SLOTS.contains(&slot.as_str()) {
                    report.skipped_availabilities += 1;
                    continue;
                }
                sqlx::query(
                    "INSERT INTO disponibilite_soumission (soumission_id, jour, slot) \
                     VALUES ($1, $2, $3)",
                )
                .bind(soumission_id)
                .bind(date)
                .bind(slot)
                .execute(&mut *tx)
                .await?;
            }

            for libelle in &row.wish_labels {
                sqlx::query(
                    "INSERT INTO soumission_libelle (soumission_id, libelle_brut) VALUES ($1, $2)",
                )
                .bind(soumission_id)
                .bind(libelle)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    tx.commit().await?;
    Ok(report)
}

async fn insert_soumission(
    tx: &mut Transaction<'_, Postgres>,
    evenement_id: Uuid,
    row: &ParsedRow,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO soumission \
            (evenement_id, nom, prenom, email, telephone, tranche_age, permis_b, taille_tshirt, date_soumission) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         ON CONFLICT (evenement_id, date_soumission, email) DO NOTHING \
         RETURNING id",
    )
    .bind(evenement_id)
    .bind(&row.nom)
    .bind(&row.prenom)
    .bind(&row.email)
    .bind(&row.telephone)
    .bind(row.tranche_age.label())
    .bind(row.permis_b)
    .bind(&row.taille_tshirt)
    .bind(row.horodateur_utc)
    .fetch_optional(&mut **tx)
    .await
}
