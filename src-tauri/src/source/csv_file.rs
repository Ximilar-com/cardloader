//! A CSV file as a session source: a cardloader export, an inventory list or a spreadsheet of image paths.
//!
//! Each row is one card. A row with a card name is an identified card and is imported as is; a row with
//! only an image path is analysed like a folder image. Columns are matched by name, loosely (`Image`,
//! `front_path`, `Card Name`, `Set`, `No.`, …), separators `,` `;` and tab are detected, and relative image
//! paths are read from the CSV's folder.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::CardFields;

const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    Front,
    Back,
    Name,
    FullName,
    SetName,
    SetCode,
    CardNumber,
    Series,
    Year,
    Category,
    IdType,
    Price,
    Grade,
    Condition,
    Distance,
}

/// Normalised header → column. The first matching header wins.
const ALIASES: &[(Column, &[&str])] = &[
    (
        Column::Front,
        &[
            "front",
            "front_path",
            "front_image",
            "image",
            "image_path",
            "image_file",
            "path",
            "file",
            "file_path",
            "filename",
            "file_name",
            "photo",
            "picture",
        ],
    ),
    (
        Column::Back,
        &["back", "back_path", "back_image", "back_file"],
    ),
    (
        Column::Name,
        &["name", "card_name", "card", "player", "title"],
    ),
    (
        Column::FullName,
        &["full_name", "fullname", "card_full_name"],
    ),
    (
        Column::SetName,
        &[
            "set_name",
            "set",
            "expansion",
            "edition",
            "release",
            "publisher",
        ],
    ),
    (
        Column::SetCode,
        &["set_code", "code", "set_id", "expansion_code"],
    ),
    (
        Column::CardNumber,
        &[
            "card_number",
            "number",
            "no",
            "card_no",
            "collector_number",
            "num",
            "card_num",
        ],
    ),
    (Column::Series, &["series", "sub_set", "subset", "brand"]),
    (Column::Year, &["year", "date", "release_year"]),
    (
        Column::Category,
        &["category", "subcategory", "game", "sport"],
    ),
    (Column::IdType, &["type", "id_type"]),
    (
        Column::Price,
        &[
            "price",
            "price_usd",
            "value",
            "market_price",
            "market_value",
            "median_price",
        ],
    ),
    (Column::Grade, &["grade", "final_grade"]),
    (Column::Condition, &["condition"]),
    (Column::Distance, &["distance"]),
];

/// One row of the file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CsvRow {
    pub front: Option<PathBuf>,
    pub back: Option<PathBuf>,
    pub card: CardFields,
    pub id_type: Option<String>,
    pub price: Option<f64>,
    pub grade: Option<f64>,
    pub condition: Option<String>,
    pub distance: Option<f64>,
}

impl CsvRow {
    /// The row names a card (it was identified elsewhere).
    pub fn has_card(&self) -> bool {
        !self.card.name.is_empty() || !self.card.full_name.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct CsvFile {
    pub rows: Vec<CsvRow>,
    /// Rows with neither a card nor an image.
    pub skipped: usize,
    /// `header → field` for the columns that were understood.
    pub columns: Vec<(String, &'static str)>,
}

/// What the import dialog shows before importing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvPreview {
    pub rows: usize,
    /// Rows naming a card.
    pub cards: usize,
    /// Rows naming a card that also have an image.
    pub cards_with_images: usize,
    /// Rows with only an image: analysed when the session runs.
    pub images: usize,
    pub skipped: usize,
    /// Image paths in the file that don't exist.
    pub missing_images: usize,
    pub columns: Vec<String>,
}

fn field_name(c: Column) -> &'static str {
    match c {
        Column::Front => "front image",
        Column::Back => "back image",
        Column::Name => "name",
        Column::FullName => "full name",
        Column::SetName => "set",
        Column::SetCode => "set code",
        Column::CardNumber => "number",
        Column::Series => "series",
        Column::Year => "year",
        Column::Category => "category",
        Column::IdType => "type",
        Column::Price => "price",
        Column::Grade => "grade",
        Column::Condition => "condition",
        Column::Distance => "distance",
    }
}

fn normalise(header: &str) -> String {
    let mut out = String::new();
    for ch in header.trim().trim_start_matches('\u{feff}').chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

/// The separator used in the header line: the most frequent of `,` `;` and tab.
fn sniff_delimiter(text: &str) -> u8 {
    let header = text.lines().next().unwrap_or_default();
    b",;\t"
        .iter()
        .copied()
        .max_by_key(|d| header.bytes().filter(|b| b == d).count())
        .unwrap_or(b',')
}

/// "$1,234.50", "12,5" (with `;` files), "€ 3" → a number.
fn parse_number(raw: &str, decimal_comma: bool) -> Option<f64> {
    let kept: String = raw
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-'))
        .collect();
    let normalised = if decimal_comma && !kept.contains('.') {
        kept.replace(',', ".")
    } else {
        kept.replace(',', "")
    };
    normalised.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn resolve(raw: &str, base: &Path) -> Option<PathBuf> {
    let raw = raw.trim().trim_matches('"');
    let raw = raw.strip_prefix("file://").unwrap_or(raw);
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    Some(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

pub fn parse(text: &str, base: &Path) -> Result<CsvFile> {
    let text = text.trim_start_matches('\u{feff}');
    let delimiter = sniff_delimiter(text);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());

    let headers = reader
        .headers()
        .map_err(|e| Error::msg(format!("not a CSV file: {e}")))?
        .clone();
    let mut map: Vec<(usize, Column)> = Vec::new();
    let mut file = CsvFile::default();
    for (column, aliases) in ALIASES {
        let found = headers.iter().enumerate().find(|(i, h)| {
            aliases.contains(&normalise(h).as_str()) && !map.iter().any(|(j, _)| j == i)
        });
        if let Some((i, header)) = found {
            map.push((i, *column));
            file.columns.push((header.to_string(), field_name(*column)));
        }
    }
    if !map
        .iter()
        .any(|(_, c)| matches!(c, Column::Front | Column::Name | Column::FullName))
    {
        return Err(Error::msg(format!(
            "no image or card name column in this CSV (found: {})",
            headers.iter().collect::<Vec<_>>().join(", ")
        )));
    }

    let decimal_comma = delimiter == b';';
    for record in reader.records() {
        let record = record.map_err(|e| Error::msg(format!("cannot read the CSV: {e}")))?;
        if file.rows.len() >= MAX_ROWS {
            return Err(Error::msg(format!(
                "the CSV has more than {MAX_ROWS} rows; split it into smaller files"
            )));
        }
        let mut row = CsvRow::default();
        for (i, column) in &map {
            let value = record.get(*i).unwrap_or_default().trim();
            if value.is_empty() {
                continue;
            }
            let text = value.to_string();
            match column {
                Column::Front => row.front = resolve(value, base),
                Column::Back => row.back = resolve(value, base),
                Column::Name => row.card.name = text,
                Column::FullName => row.card.full_name = text,
                Column::SetName => row.card.set_name = text,
                Column::SetCode => row.card.set_code = text,
                Column::CardNumber => row.card.card_number = text.trim_start_matches('#').into(),
                Column::Series => row.card.series = text,
                Column::Year => row.card.year = text,
                Column::Category => row.card.subcategory = text,
                Column::IdType => {
                    let t = text.to_lowercase();
                    if ["tcg", "sport", "comics", "slab"].contains(&t.as_str()) {
                        row.id_type = Some(t);
                    }
                }
                Column::Price => {
                    row.price = parse_number(value, decimal_comma).filter(|p| *p >= 0.0)
                }
                Column::Grade => {
                    row.grade =
                        parse_number(value, decimal_comma).filter(|g| (0.0..=10.0).contains(g))
                }
                Column::Condition => row.condition = Some(text),
                Column::Distance => {
                    row.distance = parse_number(value, decimal_comma).filter(|d| *d >= 0.0)
                }
            }
        }
        if row.card.full_name.is_empty() && !row.card.name.is_empty() {
            row.card.full_name = [
                row.card.name.as_str(),
                row.card.set_name.as_str(),
                &if row.card.card_number.is_empty() {
                    String::new()
                } else {
                    format!("#{}", row.card.card_number)
                },
            ]
            .iter()
            .filter(|p| !p.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        }
        if row.card.name.is_empty() && !row.card.full_name.is_empty() {
            row.card.name = row.card.full_name.clone();
        }
        if row.has_card() || row.front.is_some() {
            file.rows.push(row);
        } else {
            file.skipped += 1;
        }
    }
    Ok(file)
}

pub fn read(path: &Path) -> Result<CsvFile> {
    let size = std::fs::metadata(path)?.len();
    if size > MAX_BYTES {
        return Err(Error::msg("the CSV is too large (over 64 MB)"));
    }
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8(bytes).or_else(|e| {
        // Older Excel saves Windows-1252; keep what is readable.
        Ok::<_, Error>(String::from_utf8_lossy(e.as_bytes()).into_owned())
    })?;
    parse(&text, path.parent().unwrap_or(Path::new(".")))
}

pub fn preview(path: &Path) -> Result<CsvPreview> {
    let file = read(path)?;
    let missing = |p: &Option<PathBuf>| p.as_ref().is_some_and(|p| !p.is_file());
    Ok(CsvPreview {
        rows: file.rows.len() + file.skipped,
        cards: file.rows.iter().filter(|r| r.has_card()).count(),
        cards_with_images: file
            .rows
            .iter()
            .filter(|r| r.has_card() && r.front.is_some())
            .count(),
        images: file.rows.iter().filter(|r| !r.has_card()).count(),
        skipped: file.skipped,
        missing_images: file
            .rows
            .iter()
            .filter(|r| missing(&r.front) || missing(&r.back))
            .count(),
        columns: file
            .columns
            .iter()
            .map(|(h, f)| format!("{h} → {f}"))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_cardloader_export() {
        let text = "\u{feff}#,status,type,name,full_name,set_name,set_code,card_number,series,year,category,confidence,distance,picked,price_usd,grade,condition,front,back,error\r\n\
            1,done,tcg,Charizard ex,\"Charizard ex Obsidian Flames (OBF) #125\",Obsidian Flames,OBF,125,,2023,Pokemon,high,0.12,,24.99,9,Mint,cards/a.jpg,,\r\n\
            2,queued,,,,,,,,,,,,,,,,/abs/b.jpg,/abs/b2.jpg,\r\n\
            3,error,,,,,,,,,,,,,,,,,,boom\r\n";
        let file = parse(text, Path::new("/data")).unwrap();
        assert_eq!(file.rows.len(), 2);
        assert_eq!(file.skipped, 1);
        let a = &file.rows[0];
        assert!(a.has_card());
        assert_eq!(a.card.full_name, "Charizard ex Obsidian Flames (OBF) #125");
        assert_eq!(a.card.subcategory, "Pokemon");
        assert_eq!(a.id_type.as_deref(), Some("tcg"));
        assert_eq!(
            (a.price, a.grade, a.distance),
            (Some(24.99), Some(9.0), Some(0.12))
        );
        assert_eq!(a.front.as_deref(), Some(Path::new("/data/cards/a.jpg")));
        let b = &file.rows[1];
        assert!(!b.has_card());
        assert_eq!(b.back.as_deref(), Some(Path::new("/abs/b2.jpg")));
    }

    #[test]
    fn reads_a_semicolon_inventory_with_loose_headers() {
        let text = "Card Name;Set;No.;Market Price;Condition\nBlack Lotus;Alpha;#232;\"18 500,50 €\";Near Mint\n;;;;\n";
        let file = parse(text, Path::new("/")).unwrap();
        assert_eq!(file.rows.len(), 1);
        assert_eq!(file.skipped, 1);
        let r = &file.rows[0];
        assert_eq!(r.card.name, "Black Lotus");
        assert_eq!(r.card.card_number, "232");
        assert_eq!(r.card.full_name, "Black Lotus Alpha #232");
        assert_eq!(r.price, Some(18500.5));
        assert_eq!(r.condition.as_deref(), Some("Near Mint"));
        assert!(r.front.is_none());
    }

    #[test]
    fn image_lists_and_bad_files() {
        let file = parse("image\nx.png\n\n", Path::new("/in")).unwrap();
        assert_eq!(file.rows[0].front.as_deref(), Some(Path::new("/in/x.png")));
        assert!(parse("foo,bar\n1,2\n", Path::new("/")).is_err());
        assert_eq!(parse_number("$1,234.50", false), Some(1234.5));
        assert_eq!(parse_number("n/a", false), None);
        assert_eq!(normalise(" Collector Number "), "collector_number");
    }
}
