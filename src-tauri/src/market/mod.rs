//! Files for other marketplaces: a TCGplayer list of ungraded cards ([`tcgplayer`]), Whatnot's bulk listing CSV
//! ([`whatnot`]) and Shopify's product and inventory CSVs ([`shopify`]). Pictures in the files are links, so they
//! go to a public image host first ([`imgbb`]).
//!
//! A card is typed, its slab read and its title made as for eBay (`crate::ebay::listing`).

pub mod imgbb;
pub mod shopify;
pub mod tcgplayer;
pub mod whatnot;

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::Result;
use crate::imaging;
use crate::model::{CardCondition, ItemDetail, ItemSummary};
use crate::ximilar::http::ApiError;
use crate::ximilar::recognition;

/// A card to export, at the price the user set.
#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricedItem {
    pub item_id: i64,
    pub price: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Tcgplayer,
    Whatnot,
    Shopify,
}

/// The card's pictures to list: the front (its preview), and the back when there is one.
pub fn pictures(item: &ItemDetail) -> Vec<PathBuf> {
    let front = item
        .preview_path
        .clone()
        .unwrap_or_else(|| item.summary.front_path.clone());
    std::iter::once(front)
        .chain(item.back_preview_path.clone())
        .map(PathBuf::from)
        .collect()
}

pub async fn read_picture(path: PathBuf) -> std::result::Result<Vec<u8>, ApiError> {
    tokio::task::spawn_blocking(move || imaging::listing_jpeg(&path))
        .await
        .map_err(|e| ApiError::Decode(e.to_string()))?
        .map_err(|e| ApiError::Decode(format!("the picture can't be read: {e}")))
}

/// The card's condition as Ximilar's grading named it, if it graded the condition.
pub fn graded_condition(item: &ItemSummary) -> Option<CardCondition> {
    item.condition.as_deref().and_then(CardCondition::read)
}

/// The card is in a graded slab whose label was read.
pub fn slabbed(item: &ItemSummary) -> bool {
    item.slab
        .as_ref()
        .is_some_and(|s| !s.company.trim().is_empty() || !s.grade.trim().is_empty())
}

/// TCGplayer's product id, from the TCGplayer link Ximilar gives with the match.
pub fn tcgplayer_product(item: &ItemDetail) -> Option<String> {
    let raw = item.recognition.as_ref()?;
    let parsed = recognition::parse(recognition::Endpoint::of_raw(raw), &raw["response"]);
    let url = parsed.links.get("tcgplayer.com")?.as_str()?;
    let id: String = url
        .split("/product/")
        .nth(1)?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (!id.is_empty()).then_some(id)
}

/// Quotes a field when it holds a separator, quote or line break (RFC 4180).
pub fn field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub fn csv_line(fields: &[String]) -> String {
    let mut line = fields
        .iter()
        .map(|f| field(f))
        .collect::<Vec<_>>()
        .join(",");
    line.push_str("\r\n");
    line
}

/// `bom`: a byte-order mark so Excel reads the file as UTF-8; importers may read it as part of the first header.
pub fn write(path: &Path, text: &str, bom: bool) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    if bom {
        file.write_all(b"\xEF\xBB\xBF")?;
    }
    file.write_all(text.as_bytes())?;
    Ok(())
}

/// Why a card was left out of an export.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub seq: i64,
    pub reason: String,
}

/// A Magic card identified with its TCGplayer link (tests).
#[cfg(test)]
pub fn test_item(id_type: &str, game: &str) -> ItemDetail {
    use crate::model::{CardFields, ItemStatus};
    use serde_json::json;
    ItemDetail {
        summary: ItemSummary {
            id: 1,
            session_id: "s".into(),
            seq: 4,
            status: ItemStatus::Done,
            front_path: "/cards/a.jpg".into(),
            back_path: None,
            thumb_path: None,
            id_type: Some(id_type.into()),
            card: CardFields {
                name: "Leapfrog".into(),
                full_name: "Leapfrog GRN 42".into(),
                set_name: "Guilds of Ravnica".into(),
                card_number: "42".into(),
                year: "2018".into(),
                subcategory: game.into(),
                ..Default::default()
            },
            confidence: Some("high".into()),
            distance: Some(0.1),
            price: Some(0.234),
            grade: None,
            condition: None,
            slab: None,
            error: None,
            synced: false,
            updated_at: String::new(),
            picked: false,
            edited: false,
            reviewed: false,
            agent_reviewed: false,
            review_note: String::new(),
            notes: String::new(),
            location: String::new(),
            ebay: None,
        },
        event_id: "e1".into(),
        preview_path: None,
        back_preview_path: None,
        recognition: Some(
            json!({"endpoint": "collectibles/v2/tcg_id", "response": {"records": [{"_objects": [
            {"name": "Card", "_identification": {"best_match": {"name": "Leapfrog",
                "links": {"tcgplayer.com": "https://www.tcgplayer.com/product/176868"}},
             "distances": [0.1]}}]}]}}),
        ),
        grading: None,
        grading_images: vec![],
        calls: 1,
        analyzed_at: None,
        remote_id: None,
        ebay_sku: None,
        ebay_offer_id: None,
        ebay_images: vec![],
        hosted_images: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions_are_read_from_any_scale() {
        assert_eq!(
            CardCondition::read("Near Mint"),
            Some(CardCondition::NearMint)
        );
        assert_eq!(
            CardCondition::read("Lightly Played"),
            Some(CardCondition::Excellent)
        );
        assert_eq!(
            CardCondition::read("Raw - Very Good"),
            Some(CardCondition::VeryGood)
        );
        assert_eq!(CardCondition::read("damaged"), Some(CardCondition::Poor));
        // Every label of the local model's scales.
        for scale in crate::local::grade::SCALES {
            for (label, _, _) in scale.labels {
                assert!(CardCondition::read(label).is_some(), "{label}");
            }
        }
        assert_eq!(
            CardCondition::read("Moderately Played (Very Good)"),
            Some(CardCondition::VeryGood)
        );
        assert_eq!(CardCondition::read("9.5"), None);
    }
}
