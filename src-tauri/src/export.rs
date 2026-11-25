//! A session's cards as CSV, for spreadsheets and inventory tools.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::model::ItemSummary;

/// A column of the CSV, named as its header says (`#` is `seq`). The user picks which and in what order
/// (`Settings::csv_columns`); a name of a newer version reads as `Unknown` and is left out.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CsvColumn {
    Seq,
    Status,
    Type,
    Name,
    FullName,
    SetName,
    SetCode,
    CardNumber,
    Series,
    Year,
    Category,
    Confidence,
    Distance,
    Picked,
    Price,
    Currency,
    Grade,
    Condition,
    SlabCompany,
    SlabGrade,
    SlabCertificate,
    Front,
    Back,
    Error,
    Notes,
    Location,
    Brand,
    Team,
    Serial,
    Features,
    ListingTitle,
    #[serde(other)]
    #[schemars(skip)]
    Unknown,
}

use CsvColumn as C;

/// Every column, in the order a file has them unless the user chose another.
pub const ALL: &[CsvColumn] = &[
    C::Seq,
    C::Status,
    C::Type,
    C::Name,
    C::FullName,
    C::SetName,
    C::SetCode,
    C::CardNumber,
    C::Series,
    C::Year,
    C::Category,
    C::Confidence,
    C::Distance,
    C::Picked,
    C::Price,
    C::Currency,
    C::Grade,
    C::Condition,
    C::SlabCompany,
    C::SlabGrade,
    C::SlabCertificate,
    C::Front,
    C::Back,
    C::Error,
    C::Notes,
    C::Location,
    C::Brand,
    C::Team,
    C::Serial,
    C::Features,
    C::ListingTitle,
];

impl CsvColumn {
    pub fn header(self) -> &'static str {
        match self {
            C::Seq => "#",
            C::Status => "status",
            C::Type => "type",
            C::Name => "name",
            C::FullName => "full_name",
            C::SetName => "set_name",
            C::SetCode => "set_code",
            C::CardNumber => "card_number",
            C::Series => "series",
            C::Year => "year",
            C::Category => "category",
            C::Confidence => "confidence",
            C::Distance => "distance",
            C::Picked => "picked",
            C::Price => "price",
            C::Currency => "currency",
            C::Grade => "grade",
            C::Condition => "condition",
            C::SlabCompany => "slab_company",
            C::SlabGrade => "slab_grade",
            C::SlabCertificate => "slab_certificate",
            C::Front => "front",
            C::Back => "back",
            C::Error => "error",
            C::Notes => "notes",
            C::Location => "location",
            C::Brand => "brand",
            C::Team => "team",
            C::Serial => "serial",
            C::Features => "features",
            C::ListingTitle => "listing_title",
            C::Unknown => "",
        }
    }

    fn value(self, i: &ItemSummary) -> String {
        let c = &i.card;
        let slab = || i.slab.clone().unwrap_or_default();
        match self {
            C::Seq => i.seq.to_string(),
            C::Status => i.status.as_str().to_string(),
            C::Type => i.id_type.clone().unwrap_or_default(),
            C::Name => c.name.clone(),
            C::FullName => c.full_name.clone(),
            C::SetName => c.set_name.clone(),
            C::SetCode => c.set_code.clone(),
            C::CardNumber => c.card_number.clone(),
            C::Series => c.series.clone(),
            C::Year => c.year.clone(),
            C::Category => c.subcategory.clone(),
            C::Confidence => i.confidence.clone().unwrap_or_default(),
            C::Distance => number(i.distance),
            C::Picked => if i.picked { "yes" } else { "" }.to_string(),
            C::Price => number(i.price),
            C::Currency => {
                if i.price.is_some() {
                    i.price_currency.clone()
                } else {
                    String::new()
                }
            }
            C::Grade => number(i.grade),
            C::Condition => i.condition.clone().unwrap_or_default(),
            C::SlabCompany => slab().company,
            C::SlabGrade => slab().grade,
            C::SlabCertificate => slab().certificate,
            C::Front => i.front_path.clone(),
            C::Back => i.back_path.clone().unwrap_or_default(),
            C::Error => i.error.clone().unwrap_or_default(),
            C::Notes => i.notes.clone(),
            C::Location => i.location.clone(),
            C::Brand => c.brand.clone(),
            C::Team => c.team.clone(),
            C::Serial => c.serial.clone(),
            C::Features => c.features.clone(),
            // Only a title someone wrote: the automatic one depends on the templates of the day.
            C::ListingTitle => i.title.clone(),
            C::Unknown => String::new(),
        }
    }
}

/// The columns a file gets: those chosen, each once, in their order; none chosen = all of them.
pub fn columns(chosen: &[CsvColumn]) -> Vec<CsvColumn> {
    let mut out: Vec<CsvColumn> = Vec::new();
    for c in chosen {
        if *c != C::Unknown && !out.contains(c) {
            out.push(*c);
        }
    }
    if out.is_empty() {
        ALL.to_vec()
    } else {
        out
    }
}

/// Quotes a field when it holds a separator, quote or line break (RFC 4180).
fn field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn number(value: Option<f64>) -> String {
    value
        .filter(|v| v.is_finite())
        .map(|v| v.to_string())
        .unwrap_or_default()
}

/// Headers the user wrote for columns ("name" → "primary name"), as `Settings::csv_headers` keeps them.
pub type Headers = BTreeMap<CsvColumn, String>;

/// The column's header in a file: the user's name for it, else its own.
pub fn header_of(column: CsvColumn, headers: &Headers) -> String {
    match headers
        .get(&column)
        .map(|h| h.split_whitespace().collect::<Vec<_>>().join(" "))
    {
        Some(header) if !header.is_empty() => header,
        _ => column.header().to_string(),
    }
}

/// The user's headers as they are kept: trimmed, none empty or the same as the column's own, none for `Unknown`.
pub fn renamed(headers: Headers) -> Headers {
    headers
        .into_iter()
        .filter(|(c, _)| *c != C::Unknown)
        .map(|(c, h)| (c, header_of(c, &BTreeMap::from([(c, h)]))))
        .filter(|(c, h)| h != c.header())
        .collect()
}

/// The cards as CSV with the `chosen` columns (see `columns`), headed as `headers` names them.
pub fn csv(items: &[ItemSummary], chosen: &[CsvColumn], headers: &Headers) -> String {
    let columns = columns(chosen);
    let line = |values: Vec<String>| {
        let mut line = values
            .iter()
            .map(|v| field(v))
            .collect::<Vec<_>>()
            .join(",");
        line.push_str("\r\n");
        line
    };
    let mut out = line(columns.iter().map(|c| header_of(*c, headers)).collect());
    for i in items {
        out.push_str(&line(columns.iter().map(|c| c.value(i)).collect()));
    }
    out
}

pub fn write_csv(
    items: &[ItemSummary],
    path: &Path,
    chosen: &[CsvColumn],
    headers: &Headers,
) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    // A BOM so Excel reads the file as UTF-8.
    file.write_all(b"\xEF\xBB\xBF")?;
    file.write_all(csv(items, chosen, headers).as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CardFields, ItemStatus, SlabLabel};

    /// The export dialog offers the columns from `src/lib/csvColumns.ts`: the same, in the same order.
    #[test]
    fn the_dialogs_columns_are_these() {
        let ts = include_str!("../../src/lib/csvColumns.ts");
        let start = ts.find("CSV_COLUMNS: CsvColumn[] = [").unwrap();
        let list = &ts[start..start + ts[start..].find("];").unwrap()];
        let names: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
        let ours: Vec<String> = ALL
            .iter()
            .map(|c| {
                serde_json::to_value(c)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(names, ours);
    }

    #[test]
    fn quotes_what_needs_quoting() {
        let item = ItemSummary {
            id: 1,
            session_id: "s".into(),
            seq: 3,
            status: ItemStatus::Done,
            front_path: "/cards/a, front.jpg".into(),
            back_path: None,
            thumb_path: None,
            id_type: Some("tcg".into()),
            card: CardFields {
                name: "Say \"hi\"".into(),
                brand: "Topps".into(),
                features: "RC, Auto".into(),
                ..Default::default()
            },
            confidence: Some("high".into()),
            distance: Some(0.25),
            price: Some(12.5),
            price_currency: "USD".into(),
            grade: None,
            condition: None,
            slab: Some(SlabLabel {
                company: "PSA".into(),
                grade: "10".into(),
                certificate: "12345678".into(),
            }),
            error: None,
            synced: true,
            updated_at: String::new(),
            picked: true,
            edited: false,
            reviewed: false,
            agent_reviewed: false,
            review_note: String::new(),
            notes: String::new(),
            location: String::new(),
            title: "Say hi RC".into(),
            stage: None,
            ebay: None,
            market_range: None,
        };
        let text = csv(std::slice::from_ref(&item), &[], &Headers::new());
        let mut lines = text.lines();
        assert!(lines.next().unwrap().starts_with("#,status,type,name"));
        let row = lines.next().unwrap();
        assert!(row.starts_with("3,done,tcg,\"Say \"\"hi\"\"\","));
        assert!(row.contains(",high,0.25,yes,12.5,USD,,,PSA,10,12345678,\"/cards/a, front.jpg\",,"));
        assert!(text
            .lines()
            .next()
            .unwrap()
            .ends_with(",location,brand,team,serial,features,listing_title"));
        assert!(row.ends_with(",Topps,,,\"RC, Auto\",Say hi RC"));

        // The columns chosen, in their order, each once; one of a newer version is left out.
        let chosen = [
            C::ListingTitle,
            C::Unknown,
            C::Name,
            C::Price,
            C::Name,
            C::Seq,
        ];
        assert_eq!(
            csv(std::slice::from_ref(&item), &chosen, &Headers::new()),
            "listing_title,name,price,#\r\nSay hi RC,\"Say \"\"hi\"\"\",12.5,3\r\n"
        );
        // Headers the user wrote; one left empty keeps the column's own, one with a comma is quoted.
        let headers = Headers::from([
            (C::Name, "  primary   name ".to_string()),
            (C::Price, "price, USD".to_string()),
            (C::Seq, " ".to_string()),
        ]);
        assert_eq!(
            csv(&[item], &[C::Name, C::Price, C::Seq], &headers),
            "primary name,\"price, USD\",#\r\n\"Say \"\"hi\"\"\",12.5,3\r\n"
        );
        assert_eq!(
            renamed(Headers::from([
                (C::Name, " primary name".to_string()),
                (C::Seq, "".to_string()),
                (C::Year, "year".to_string()),
                (C::Unknown, "x".to_string()),
            ])),
            Headers::from([(C::Name, "primary name".to_string())])
        );
        assert_eq!(columns(&[C::Unknown]), ALL);
        assert_eq!(
            serde_json::from_str::<Vec<CsvColumn>>(r#"["seq", "listing_title", "hologram"]"#)
                .unwrap(),
            [C::Seq, C::ListingTitle, C::Unknown]
        );
    }
}
