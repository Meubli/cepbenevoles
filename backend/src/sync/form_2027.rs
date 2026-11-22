//! Column layout for the 2027 "APPEL A BENEVOLES - FRANCES 2027" form.
//!
//! Hardcoded on purpose: next year's events will very likely come with a
//! different Google Form (different questions, different column order,
//! maybe different day labels). Supporting that is a **code change** to add
//! a new `form_YYYY` module, not a configuration screen — not worth
//! building a generic column-mapping UI for a single-admin, once-a-year
//! form. See specs.md §3.3 (per-event `sheet_id`) for how an event picks
//! which form/parser it uses.

use chrono::{NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Europe::Paris;

use super::{split_checkbox_cell, EventAttendance, EventSlug, ParsedRow, RowError, TrancheAge};

const COL_HORODATEUR: &str = "Horodateur";
const COL_NOM: &str = "Nom";
const COL_PRENOM: &str = "Prénom";
const COL_TRANCHE_AGE: &str = "Tranche d'âge (en avril 2027)";
const COL_MAIL: &str = "Mail";
const COL_TELEPHONE: &str = "Numéro de téléphone";
const COL_PERMIS_B: &str = "Êtes-vous titulaire du permis B ?";
const COL_TAILLE_TSHIRT: &str = "Taille de Tee-Shirt (pour briller le jour-J ✨)";

const COL_SEREZ_INDIV: &str =
    "📍 Serez vous de la partie pour les  Championnats de France Individuel Perf GAM/GAF ?";
const COL_SEREZ_TEAMGYM: &str =
    "📍  Serez vous de la partie pour les Championnats de France TeamGym Performance  ?";

/// Radio-button value meaning "not attending", identical for both events —
/// relying on this fixed string is more robust than matching the two
/// slightly different "yes" wordings ("Bien sûr, je suis…" vs "Bien sûr je
/// suis…", punctuation differs).
const NOT_ATTENDING: &str = "Pas de la partie";

const COL_POSTE_10_12: &str = "Poste(s) souhaité(s) (pour les 10 - 12 ans)";
const COL_POSTE_13_14: &str = "Poste(s) souhaité(s) (pour les 13 - 14 ans)";
const COL_POSTE_15_PLUS: &str = "Poste(s) souhaité(s) (pour les + de 15 ans)";

/// The one tab this form's responses Sheet has (cf. specs.md §3.3).
pub const TAB_NAME: &str = "Réponses au formulaire 1";

/// Maps an `Evenement.bloc_formulaire` value (admin-configured, Événements
/// screen) to the [`EventSlug`] this parser knows how to read from a row.
/// Unknown values are a config error at the orchestration level, not a
/// parser one — the admin picked a value this code doesn't recognize.
pub fn bloc_to_slug(bloc: &str) -> Option<EventSlug> {
    match bloc {
        "form_2027_indiv" => Some(EventSlug::IndivGamGaf),
        "form_2027_teamgym" => Some(EventSlug::TeamGym),
        _ => None,
    }
}

/// One form day column: its header text, the day-name prefix to strip from
/// each checkbox token (e.g. "Jeudi matin" -> "matin"), and the real
/// calendar date it corresponds to.
struct DayColumn {
    header: &'static str,
    day_prefix: &'static str,
    date: NaiveDate,
}

fn indiv_days() -> Vec<DayColumn> {
    vec![
        DayColumn {
            header: "➡️ Disponibilités JEUDI 29 avril",
            day_prefix: "Jeudi",
            date: NaiveDate::from_ymd_opt(2027, 4, 29).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilités VENDREDI 30 avril",
            day_prefix: "Vendredi",
            date: NaiveDate::from_ymd_opt(2027, 4, 30).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilités SAMEDI 1e mai",
            day_prefix: "Samedi",
            date: NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilités DIMANCHE 2 mai",
            day_prefix: "Dimanche",
            date: NaiveDate::from_ymd_opt(2027, 5, 2).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilités LUNDI 3 mai",
            day_prefix: "Lundi",
            date: NaiveDate::from_ymd_opt(2027, 5, 3).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilité MARDI 4 mai",
            day_prefix: "Mardi",
            date: NaiveDate::from_ymd_opt(2027, 5, 4).unwrap(),
        },
    ]
}

fn teamgym_days() -> Vec<DayColumn> {
    vec![
        DayColumn {
            header: "➡️ Disponibilités VENDREDI 11 juin",
            day_prefix: "Vendredi",
            date: NaiveDate::from_ymd_opt(2027, 6, 11).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilités SAMEDI 12 juin",
            day_prefix: "Samedi",
            date: NaiveDate::from_ymd_opt(2027, 6, 12).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilités DIMANCHE 13 juin",
            day_prefix: "Dimanche",
            date: NaiveDate::from_ymd_opt(2027, 6, 13).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilités LUNDI 14 juin",
            day_prefix: "Lundi",
            date: NaiveDate::from_ymd_opt(2027, 6, 14).unwrap(),
        },
        DayColumn {
            header: "➡️ Disponibilité MARDI 15 juin",
            day_prefix: "Mardi",
            date: NaiveDate::from_ymd_opt(2027, 6, 15).unwrap(),
        },
    ]
}

fn normalize_header(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn header_index(header: &[String], name: &str) -> Option<usize> {
    let target = normalize_header(name);
    header.iter().position(|h| normalize_header(h) == target)
}

/// Every column this parser reads, by name. Checked once per Sheet before
/// parsing any row — a renamed question (it has happened: Déroulement ->
/// Plateau) would otherwise make every row "parse successfully" with that
/// column silently treated as absent (empty availabilities, no attendance),
/// reporting 0 inserted with no error anywhere.
pub fn validate_header(header: &[String]) -> Result<(), Vec<&'static str>> {
    let mut required: Vec<&'static str> = vec![
        COL_HORODATEUR,
        COL_NOM,
        COL_PRENOM,
        COL_TRANCHE_AGE,
        COL_MAIL,
        COL_TELEPHONE,
        COL_PERMIS_B,
        COL_TAILLE_TSHIRT,
        COL_SEREZ_INDIV,
        COL_SEREZ_TEAMGYM,
        COL_POSTE_10_12,
        COL_POSTE_13_14,
        COL_POSTE_15_PLUS,
    ];
    required.extend(indiv_days().iter().map(|d| d.header));
    required.extend(teamgym_days().iter().map(|d| d.header));

    let missing: Vec<&'static str> = required
        .into_iter()
        .filter(|col| header_index(header, col).is_none())
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

fn cell(row: &[String], idx: usize) -> &str {
    row.get(idx).map(String::as_str).unwrap_or("").trim()
}

fn required_field(
    row: &[String],
    header: &[String],
    row_index: usize,
    column: &'static str,
) -> Result<String, RowError> {
    let idx = header_index(header, column).ok_or(RowError::MissingField {
        row_index,
        field: column,
    })?;
    let value = cell(row, idx);
    if value.is_empty() {
        return Err(RowError::MissingField {
            row_index,
            field: column,
        });
    }
    Ok(value.to_string())
}

fn parse_tranche_age(row_index: usize, value: &str) -> Result<TrancheAge, RowError> {
    match value {
        "10 - 12 ans" => Ok(TrancheAge::Age10To12),
        "13 - 14 ans" => Ok(TrancheAge::Age13To14),
        "15 - 17 ans" => Ok(TrancheAge::Age15To17),
        "18 ans et +" => Ok(TrancheAge::Age18Plus),
        other => Err(RowError::UnrecognizedValue {
            row_index,
            field: COL_TRANCHE_AGE,
            value: other.to_string(),
        }),
    }
}

fn parse_permis_b(row_index: usize, value: &str) -> Result<bool, RowError> {
    match value {
        "Oui" => Ok(true),
        "Non" => Ok(false),
        other => Err(RowError::UnrecognizedValue {
            row_index,
            field: COL_PERMIS_B,
            value: other.to_string(),
        }),
    }
}

/// Parses "DD/MM/YYYY HH:MM:SS" as Europe/Paris local time (the Sheet's
/// locale, cf. technical-specs.md §7) and converts to UTC.
fn parse_horodateur(
    row_index: usize,
    value: &str,
) -> Result<chrono::DateTime<chrono::Utc>, RowError> {
    let naive = NaiveDateTime::parse_from_str(value, "%d/%m/%Y %H:%M:%S").map_err(|_| {
        RowError::InvalidHorodateur {
            row_index,
            value: value.to_string(),
        }
    })?;

    let paris_dt = match Paris.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt,
        // Ambiguous (DST fall-back, the hour repeats): pick the earlier
        // occurrence deterministically rather than guessing.
        chrono::LocalResult::Ambiguous(earliest, _latest) => earliest,
        // None (DST spring-forward gap, that local time never happened):
        // reject rather than silently shifting it.
        chrono::LocalResult::None => {
            return Err(RowError::InvalidHorodateur {
                row_index,
                value: value.to_string(),
            })
        }
    };

    Ok(paris_dt.with_timezone(&chrono::Utc))
}

fn parse_day_availabilities(
    row: &[String],
    header: &[String],
    days: &[DayColumn],
) -> Vec<(NaiveDate, String)> {
    let mut out = Vec::new();
    for day in days {
        let Some(idx) = header_index(header, day.header) else {
            continue;
        };
        let value = cell(row, idx);
        for token in split_checkbox_cell(value) {
            let slot = token
                .strip_prefix(day.day_prefix)
                .map(str::trim)
                .unwrap_or(&token);
            out.push((day.date, slot.to_string()));
        }
    }
    out
}

fn parse_attendance(
    row: &[String],
    header: &[String],
    evenement: EventSlug,
    serez_col: &'static str,
    days: &[DayColumn],
) -> Option<EventAttendance> {
    let idx = header_index(header, serez_col)?;
    let value = cell(row, idx);
    if value.is_empty() || value == NOT_ATTENDING {
        return None;
    }
    Some(EventAttendance {
        evenement,
        availabilities: parse_day_availabilities(row, header, days),
    })
}

/// Parses one data row against the 2027 form's header row.
///
/// `row_index` is the caller's position of this row in the full Sheet
/// values array, used only to identify which row a `RowError` came from.
/// The dedup key is `(horodateur, email)` (cf. specs.md §3.3), not the row
/// position — a position-based key would break as soon as a row is
/// reordered or deleted in the Sheet.
pub fn parse_row(
    header: &[String],
    row: &[String],
    row_index: usize,
) -> Result<ParsedRow, RowError> {
    let horodateur_raw = required_field(row, header, row_index, COL_HORODATEUR)?;
    let horodateur_utc = parse_horodateur(row_index, &horodateur_raw)?;

    let nom = required_field(row, header, row_index, COL_NOM)?;
    let prenom = required_field(row, header, row_index, COL_PRENOM)?;
    let email = required_field(row, header, row_index, COL_MAIL)?;
    let telephone = required_field(row, header, row_index, COL_TELEPHONE)?;
    let taille_tshirt = required_field(row, header, row_index, COL_TAILLE_TSHIRT)?;

    let tranche_age = parse_tranche_age(
        row_index,
        &required_field(row, header, row_index, COL_TRANCHE_AGE)?,
    )?;
    let permis_b = parse_permis_b(
        row_index,
        &required_field(row, header, row_index, COL_PERMIS_B)?,
    )?;

    let mut attendances = Vec::new();
    if let Some(a) = parse_attendance(
        row,
        header,
        EventSlug::IndivGamGaf,
        COL_SEREZ_INDIV,
        &indiv_days(),
    ) {
        attendances.push(a);
    }
    if let Some(a) = parse_attendance(
        row,
        header,
        EventSlug::TeamGym,
        COL_SEREZ_TEAMGYM,
        &teamgym_days(),
    ) {
        attendances.push(a);
    }

    // Union of the three age-bracket poste columns — only one is normally
    // populated (the one matching the respondent's tranche), but 15-17 and
    // 18+ share the same column, so take whichever has content rather than
    // picking by tranche.
    let mut wish_labels = Vec::new();
    for col in [COL_POSTE_10_12, COL_POSTE_13_14, COL_POSTE_15_PLUS] {
        if let Some(idx) = header_index(header, col) {
            wish_labels.extend(split_checkbox_cell(cell(row, idx)));
        }
    }

    Ok(ParsedRow {
        row_index,
        horodateur_utc,
        nom,
        prenom,
        email,
        telephone,
        tranche_age,
        permis_b,
        taille_tshirt,
        attendances,
        wish_labels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_header_accepts_the_real_fixture_header() {
        let (header, _) = fixture();
        assert_eq!(validate_header(&header), Ok(()));
    }

    #[test]
    fn validate_header_reports_a_renamed_column() {
        let (mut header, _) = fixture();
        // Simulate the association renaming a question, as already
        // happened once (Déroulement -> Plateau, cf. specs.md §3.1).
        let idx = header_index(&header, COL_SEREZ_INDIV).unwrap();
        header[idx] = "Un intitulé renommé".to_string();

        let missing = validate_header(&header).unwrap_err();
        assert_eq!(missing, vec![COL_SEREZ_INDIV]);
    }

    #[test]
    fn bloc_to_slug_maps_known_values_and_rejects_unknown() {
        assert_eq!(
            bloc_to_slug("form_2027_indiv"),
            Some(EventSlug::IndivGamGaf)
        );
        assert_eq!(bloc_to_slug("form_2027_teamgym"), Some(EventSlug::TeamGym));
        assert_eq!(bloc_to_slug("form_2028_indiv"), None);
    }

    fn fixture() -> (Vec<String>, Vec<Vec<String>>) {
        let raw: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/sheets_responses_form1.json"
        ))
        .unwrap();
        let values = raw["values"].as_array().unwrap().clone();
        let header: Vec<String> = values[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let rows = values[1..]
            .iter()
            .map(|row| {
                row.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect()
            })
            .collect();
        (header, rows)
    }

    #[test]
    fn parses_martin_camille_transport_only_indiv() {
        let (header, rows) = fixture();
        let parsed = parse_row(&header, &rows[0], 1).expect("row should parse");

        assert_eq!(parsed.nom, "Martin");
        assert_eq!(parsed.prenom, "Camille");
        assert_eq!(parsed.tranche_age, TrancheAge::Age18Plus);
        assert!(parsed.permis_b);
        assert_eq!(parsed.wish_labels, vec!["Transport".to_string()]);

        assert_eq!(parsed.attendances.len(), 1, "TeamGym cell was empty");
        assert_eq!(parsed.attendances[0].evenement, EventSlug::IndivGamGaf);
        assert!(parsed.attendances[0].availabilities.contains(&(
            NaiveDate::from_ymd_opt(2027, 4, 29).unwrap(),
            "matin".to_string()
        )));
    }

    #[test]
    fn parses_dubois_hugo_attending_both_events() {
        let (header, rows) = fixture();
        let parsed = parse_row(&header, &rows[1], 2).expect("row should parse");

        assert_eq!(parsed.nom, "Dubois");
        assert!(!parsed.permis_b);
        assert_eq!(
            parsed.attendances.len(),
            2,
            "attends both Indiv and TeamGym"
        );

        let indiv = parsed
            .attendances
            .iter()
            .find(|a| a.evenement == EventSlug::IndivGamGaf)
            .unwrap();
        assert!(indiv.availabilities.contains(&(
            NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
            "matin".to_string()
        )));

        let teamgym = parsed
            .attendances
            .iter()
            .find(|a| a.evenement == EventSlug::TeamGym)
            .unwrap();
        assert!(teamgym.availabilities.contains(&(
            NaiveDate::from_ymd_opt(2027, 6, 11).unwrap(),
            "matin".to_string()
        )));
    }

    #[test]
    fn parses_zoe_13_14_with_poste_precis_wish() {
        let (header, rows) = fixture();
        let parsed = parse_row(&header, &rows[2], 3).expect("row should parse");

        assert_eq!(parsed.tranche_age, TrancheAge::Age13To14);
        assert!(!parsed.permis_b);
        assert_eq!(
            parsed.wish_labels,
            vec!["Secrétaire de table pour la saisie des notes".to_string()]
        );
    }

    #[test]
    fn missing_required_field_is_a_row_error_not_a_panic() {
        let (header, _) = fixture();
        let row_with_blank_permis_b: Vec<String> = vec![
            "21/11/2026 18:30:00".to_string(),
            "Lefèvre".to_string(),
            "Zoé".to_string(),
            "13 - 14 ans".to_string(),
            "zoe.lefevre.test@example.com".to_string(),
            "0600000003".to_string(),
            "".to_string(), // permis B left blank
            "S".to_string(),
        ];

        let err = parse_row(&header, &row_with_blank_permis_b, 3).unwrap_err();
        assert_eq!(
            err,
            RowError::MissingField {
                row_index: 3,
                field: COL_PERMIS_B,
            }
        );
    }

    #[test]
    fn joueur_polyvalent_wish_with_comma_is_not_split() {
        let header = vec![
            COL_HORODATEUR.to_string(),
            COL_NOM.to_string(),
            COL_PRENOM.to_string(),
            COL_TRANCHE_AGE.to_string(),
            COL_MAIL.to_string(),
            COL_TELEPHONE.to_string(),
            COL_PERMIS_B.to_string(),
            COL_TAILLE_TSHIRT.to_string(),
            COL_SEREZ_INDIV.to_string(),
            COL_POSTE_15_PLUS.to_string(),
        ];
        let row = vec![
            "22/11/2026 14:37:06".to_string(),
            "Dupont".to_string(),
            "Jean".to_string(),
            "18 ans et +".to_string(),
            "jean.dupont.test@example.com".to_string(),
            "0600000099".to_string(),
            "Oui".to_string(),
            "M".to_string(),
            "Bien sûr, je suis disponible pour vous prêter main forte".to_string(),
            "Je suis joueur, peu m'importe 😜".to_string(),
        ];

        let parsed = parse_row(&header, &row, 1).expect("row should parse");
        assert_eq!(
            parsed.wish_labels,
            vec!["Je suis joueur, peu m'importe 😜".to_string()]
        );
    }

    #[test]
    fn pas_de_la_partie_means_no_attendance_for_that_event() {
        let header = vec![
            COL_HORODATEUR.to_string(),
            COL_NOM.to_string(),
            COL_PRENOM.to_string(),
            COL_TRANCHE_AGE.to_string(),
            COL_MAIL.to_string(),
            COL_TELEPHONE.to_string(),
            COL_PERMIS_B.to_string(),
            COL_TAILLE_TSHIRT.to_string(),
            COL_SEREZ_INDIV.to_string(),
            "➡️ Disponibilités JEUDI 29 avril".to_string(),
            COL_SEREZ_TEAMGYM.to_string(),
            "➡️ Disponibilités VENDREDI 11 juin".to_string(),
        ];
        let row = vec![
            "22/11/2026 14:37:06".to_string(),
            "Durand".to_string(),
            "Alex".to_string(),
            "18 ans et +".to_string(),
            "alex.durand.test@example.com".to_string(),
            "0600000042".to_string(),
            "Oui".to_string(),
            "M".to_string(),
            "Pas de la partie".to_string(),
            "".to_string(),
            "Bien sûr je suis disponible pour vous prêter main forte".to_string(),
            "Vendredi matin".to_string(),
        ];

        let parsed = parse_row(&header, &row, 1).expect("row should parse");

        assert_eq!(parsed.attendances.len(), 1, "only TeamGym attended");
        assert_eq!(parsed.attendances[0].evenement, EventSlug::TeamGym);
    }

    #[test]
    fn invalid_horodateur_is_a_row_error() {
        let (header, rows) = fixture();
        let mut bad_row = rows[0].clone();
        bad_row[0] = "not a date".to_string();

        let err = parse_row(&header, &bad_row, 1).unwrap_err();
        assert_eq!(
            err,
            RowError::InvalidHorodateur {
                row_index: 1,
                value: "not a date".to_string(),
            }
        );
    }
}
