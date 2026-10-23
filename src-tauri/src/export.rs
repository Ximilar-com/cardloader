//! A session's cards as CSV, for spreadsheets and inventory tools.

use std::io::Write;
use std::path::Path;

use crate::error::Result;
use crate::model::ItemSummary;

const HEADER: &[&str] = &[
    "#",
    "status",
    "type",
    "name",
    "full_name",
    "set_name",
    "set_code",
    "card_number",
    "series",
    "year",
    "category",
    "confidence",
    "distance",
    "picked",
    "price_usd",
    "grade",
    "condition",
    "slab_company",
    "slab_grade",
    "slab_certificate",
    "front",
    "back",
    "error",
];

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

pub fn csv(items: &[ItemSummary]) -> String {
    let mut out = HEADER.join(",");
    out.push_str("\r\n");
    for i in items {
        let c = &i.card;
        let slab = i.slab.clone().unwrap_or_default();
        let row = [
            i.seq.to_string(),
            i.status.as_str().to_string(),
            i.id_type.clone().unwrap_or_default(),
            c.name.clone(),
            c.full_name.clone(),
            c.set_name.clone(),
            c.set_code.clone(),
            c.card_number.clone(),
            c.series.clone(),
            c.year.clone(),
            c.subcategory.clone(),
            i.confidence.clone().unwrap_or_default(),
            number(i.distance),
            if i.picked { "yes" } else { "" }.to_string(),
            number(i.price),
            number(i.grade),
            i.condition.clone().unwrap_or_default(),
            slab.company,
            slab.grade,
            slab.certificate,
            i.front_path.clone(),
            i.back_path.clone().unwrap_or_default(),
            i.error.clone().unwrap_or_default(),
        ];
        out.push_str(&row.iter().map(|v| field(v)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    out
}

pub fn write_csv(items: &[ItemSummary], path: &Path) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    // A BOM so Excel reads the file as UTF-8.
    file.write_all(b"\xEF\xBB\xBF")?;
    file.write_all(csv(items).as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CardFields, ItemStatus, SlabLabel};

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
                ..Default::default()
            },
            confidence: Some("high".into()),
            distance: Some(0.25),
            price: Some(12.5),
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
            ebay: None,
        };
        let text = csv(&[item]);
        let mut lines = text.lines();
        assert!(lines.next().unwrap().starts_with("#,status,type,name"));
        let row = lines.next().unwrap();
        assert!(row.starts_with("3,done,tcg,\"Say \"\"hi\"\"\","));
        assert!(row.contains(",high,0.25,yes,12.5,,,PSA,10,12345678,\"/cards/a, front.jpg\",,"));
    }
}
