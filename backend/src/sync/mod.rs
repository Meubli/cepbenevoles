//! Parsing of raw Google Sheets rows into domain data.
//!
//! Pure, no I/O: takes the header row and a data row (as returned by
//! [`crate::sheets::SheetsClient::fetch_values`]) and returns structured
//! data or a per-row error. No persistence, no sync scheduling, and no
//! form-label -> commission/poste mapping (cf. specs.md §3.2) here yet —
//! those are separate concerns, built on top of this.

pub mod form_2027;

use chrono::{DateTime, NaiveDate, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrancheAge {
    Age10To12,
    Age13To14,
    Age15To17,
    Age18Plus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSlug {
    IndivGamGaf,
    TeamGym,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAttendance {
    pub evenement: EventSlug,
    /// (date, slot label e.g. "matin"/"midi"/"aprem"/"soir")
    pub availabilities: Vec<(NaiveDate, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRow {
    pub row_index: usize,
    pub horodateur_utc: DateTime<Utc>,
    pub nom: String,
    pub prenom: String,
    pub email: String,
    pub telephone: String,
    pub tranche_age: TrancheAge,
    pub permis_b: bool,
    pub taille_tshirt: String,
    pub attendances: Vec<EventAttendance>,
    /// Raw, unmapped wish labels (union of the age-bracket-specific poste
    /// columns) — resolved to Commission/Poste via the Mapping form screen
    /// (specs.md §3.2), not here.
    pub wish_labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RowError {
    #[error("row {row_index}: missing required field \"{field}\"")]
    MissingField {
        row_index: usize,
        field: &'static str,
    },
    #[error("row {row_index}: unrecognized value \"{value}\" for \"{field}\"")]
    UnrecognizedValue {
        row_index: usize,
        field: &'static str,
        value: String,
    },
    #[error("row {row_index}: invalid horodateur \"{value}\"")]
    InvalidHorodateur { row_index: usize, value: String },
}

/// Splits a comma-separated checkbox cell into individual labels.
///
/// Google Forms checkbox answers are joined with `", "`, but at least one
/// real label itself contains that exact separator ("Je suis joueur, peu
/// m'importe 😜"). Every real label starts with an uppercase letter (or an
/// emoji/digit), so splitting only before an uppercase letter avoids
/// breaking that label apart. This is a heuristic, not a guarantee — see
/// specs.md §3.2.
pub(crate) fn split_checkbox_cell(cell: &str) -> Vec<String> {
    let cell = cell.trim();
    if cell.is_empty() {
        return Vec::new();
    }

    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = cell.chars().peekable();

    while let Some(c) = chars.next() {
        if c == ',' {
            // Look ahead: ", " followed by an uppercase letter starts a new
            // label; otherwise the comma is part of the current one.
            let mut lookahead = chars.clone();
            let next_is_space = lookahead.peek() == Some(&' ');
            if next_is_space {
                lookahead.next();
            }
            let starts_new_label = lookahead
                .peek()
                .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit());

            if next_is_space && starts_new_label {
                parts.push(current.trim().to_string());
                current = String::new();
                chars.next(); // consume the space we peeked at
                continue;
            }
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_checkbox_cell_keeps_the_joueur_label_intact() {
        assert_eq!(
            split_checkbox_cell("Je suis joueur, peu m'importe 😜"),
            vec!["Je suis joueur, peu m'importe 😜".to_string()]
        );
    }

    #[test]
    fn split_checkbox_cell_splits_genuine_multi_select() {
        assert_eq!(
            split_checkbox_cell("Accueil, Boutique"),
            vec!["Accueil".to_string(), "Boutique".to_string()]
        );
    }

    #[test]
    fn split_checkbox_cell_handles_empty() {
        assert_eq!(split_checkbox_cell(""), Vec::<String>::new());
        assert_eq!(split_checkbox_cell("   "), Vec::<String>::new());
    }

    #[test]
    fn split_checkbox_cell_handles_single_label() {
        assert_eq!(
            split_checkbox_cell("Transport"),
            vec!["Transport".to_string()]
        );
    }
}
