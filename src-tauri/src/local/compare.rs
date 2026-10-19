//! How well the local model does: run it on cards the Ximilar API already identified and count how often the
//! two agree. Nothing in the session changes.

use serde::Serialize;

/// One card: what Ximilar found and what the local model read.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareRow {
    pub item_id: i64,
    pub seq: i64,
    pub ximilar: String,
    pub local: String,
    pub name_matches: bool,
    pub number_matches: bool,
    pub seconds: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareReport {
    pub model: String,
    pub cards: usize,
    pub names: usize,
    pub numbers: usize,
    /// Cards where Ximilar has a collector number to compare with.
    pub with_number: usize,
    pub seconds_per_card: f64,
    pub cancelled: bool,
    pub rows: Vec<CompareRow>,
}

/// Lower case, letters and digits only: "Charizard ex" = "CHARIZARD EX", "Blue-Eyes" = "Blue Eyes".
pub fn normalise_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The number before any "/", without leading zeros: "004/325" = "4", "#58" = "58".
pub fn normalise_number(number: &str) -> String {
    let n = number.trim().trim_start_matches('#');
    let n = n.split('/').next().unwrap_or_default().trim();
    let n = n.trim_start_matches('0');
    n.to_lowercase()
}

pub fn names_match(a: &str, b: &str) -> bool {
    let (a, b) = (normalise_name(a), normalise_name(b));
    !a.is_empty() && a == b
}

pub fn numbers_match(a: &str, b: &str) -> bool {
    let (a, b) = (normalise_number(a), normalise_number(b));
    !a.is_empty() && a == b
}

impl CompareReport {
    pub fn add(&mut self, row: CompareRow, ximilar_number: &str) {
        self.cards += 1;
        self.names += row.name_matches as usize;
        if !ximilar_number.trim().is_empty() {
            self.with_number += 1;
            self.numbers += row.number_matches as usize;
        }
        self.rows.push(row);
        let total: f64 = self.rows.iter().map(|r| r.seconds).sum();
        self.seconds_per_card = (total / self.rows.len() as f64 * 100.0).round() / 100.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_is_forgiving_about_format_only() {
        assert!(names_match("Charizard ex", "CHARIZARD EX"));
        assert!(names_match(
            "Blue-Eyes White Dragon",
            "Blue Eyes White Dragon"
        ));
        assert!(!names_match("Umbreon V", "Umbreon"));
        assert!(!names_match("", ""));
        assert!(numbers_match("004/325", "4"));
        assert!(numbers_match("#58", "58/102"));
        assert!(!numbers_match("125/197", "330"));
        assert!(!numbers_match("", ""));
    }

    #[test]
    fn report_counts() {
        let row = |name_matches, number_matches| CompareRow {
            item_id: 1,
            seq: 1,
            ximilar: String::new(),
            local: String::new(),
            name_matches,
            number_matches,
            seconds: 1.0,
            error: None,
        };
        let mut report = CompareReport::default();
        report.add(row(true, true), "58/102");
        report.add(row(true, false), "");
        report.add(row(false, false), "1");
        assert_eq!(
            (
                report.cards,
                report.names,
                report.numbers,
                report.with_number
            ),
            (3, 2, 1, 2)
        );
        assert_eq!(report.seconds_per_card, 1.0);
    }
}
