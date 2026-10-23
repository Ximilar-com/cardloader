//! Whatnot's bulk listing CSV (Seller Hub → Inventory → Import from CSV, or a show's Upload CSV): the 21 columns
//! of Whatnot's US template, with its exact category, sub category and condition values.
//!
//! The pictures are links Whatnot downloads, so they must be public: they are uploaded to ImgBB first (once per
//! card, see `items.hosted_images`). Without pictures the listings import as drafts the seller adds photos to.

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::imgbb::Hosting;
use super::{csv_line, graded_condition, PricedItem, Skipped};
use crate::ebay::listing::{self, Kind, Listing, Preview};
use crate::model::{CardCondition, ItemDetail, ItemSummary};
use crate::runner::Runner;
use crate::ximilar::http::ApiError;

pub const HEADER: &[&str] = &[
    "Category",
    "Sub Category",
    "Title",
    "Description",
    "Quantity",
    "Type",
    "Price",
    "Shipping Profile",
    "Offerable",
    "Hazmat",
    "Condition",
    "Cost Per Item",
    "SKU",
    "Image URL 1",
    "Image URL 2",
    "Image URL 3",
    "Image URL 4",
    "Image URL 5",
    "Image URL 6",
    "Image URL 7",
    "Image URL 8",
];
const MAX_IMAGES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingType {
    /// The price is the starting bid; the auction runs in a show.
    #[default]
    Auction,
    BuyItNow,
}

impl ListingType {
    fn as_whatnot(self) -> &'static str {
        match self {
            ListingType::Auction => "Auction",
            ListingType::BuyItNow => "Buy it Now",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub listing_type: ListingType,
    pub shipping_profile: String,
    /// Buyers may make offers (Buy it Now).
    pub offerable: bool,
    /// For ungraded cards whose condition wasn't graded.
    pub condition: CardCondition,
    pub upload_images: bool,
}

/// Whatnot's category and sub category for the card, or why it has none.
pub fn category(item: &ItemSummary, kind: Kind) -> (&'static str, &'static str) {
    let game = item.card.subcategory.as_str();
    match kind {
        Kind::Sport => (
            "Sports Cards",
            match game {
                "Baseball" => "Baseball Singles",
                "Basketball" => "Basketball Singles",
                "Football" => "Football Singles",
                "Hockey" => "Hockey Singles",
                "Soccer" => "Soccer Singles",
                "MMA" => "UFC Singles",
                _ => "Other Sports Cards",
            },
        ),
        Kind::Tcg => match game {
            "Garbage Pail Kids" => ("Entertainment Cards", "Garbage Pail Kids"),
            "Marvel Universe" => ("Entertainment Cards", "Marvel Cards"),
            _ => (
                "Trading Card Games",
                match game {
                    "Pokemon" => "Pokémon Cards",
                    "Magic The Gathering" => "Magic: The Gathering",
                    "YuGiOh!" => "Yu-Gi-Oh! Cards",
                    "One Piece" => "One Piece Cards",
                    "Lorcana" => "Lorcana",
                    "Digimon" => "Digimon Cards",
                    "Dragon Ball Super" => "Dragon Ball Cards",
                    "Flesh and Blood" => "Flesh & Blood",
                    "MetaZoo" => "MetaZoo",
                    "Naruto Mythos" => "Naruto Cards",
                    "Union Arena" => "Union Arena",
                    "Weiss Schwarz" => "Weiß Schwarz",
                    _ => "Kickstarter & Other Cards",
                },
            ),
        },
    }
}

/// Whatnot's condition: "Graded" for slabs; raw cards on the sub category's own scale.
pub fn condition(kind: Kind, graded: bool, raw: CardCondition) -> &'static str {
    match (graded, kind, raw) {
        (true, _, _) => "Graded",
        (false, Kind::Sport, CardCondition::NearMint) => "Raw - Near Mint or Better",
        (false, Kind::Sport, CardCondition::Excellent) => "Raw - Excellent",
        (false, Kind::Sport, CardCondition::VeryGood) => "Raw - Very Good",
        (false, Kind::Sport, CardCondition::Poor) => "Raw - Poor",
        (false, Kind::Tcg, CardCondition::NearMint) => "Near Mint",
        (false, Kind::Tcg, CardCondition::Excellent) => "Light Played",
        (false, Kind::Tcg, CardCondition::VeryGood) => "Moderately Played",
        (false, Kind::Tcg, CardCondition::Poor) => "Heavily Played",
    }
}

/// Whole dollars, rounded up (Whatnot's import takes no cents).
pub fn whole_dollars(price: f64) -> u64 {
    price.ceil().max(1.0) as u64
}

/// Plain text: Whatnot shows the description as written. The slab's grader and grade go here (the CSV has no
/// field for them).
pub fn description(item: &ItemSummary, plan: &Listing) -> String {
    let card = &item.card;
    let mut lines = vec![plan.title.clone()];
    for (label, value) in [
        ("Set", &card.set_name),
        ("Number", &card.card_number),
        ("Year", &card.year),
    ] {
        if !value.trim().is_empty() {
            lines.push(format!("{label}: {}", value.trim()));
        }
    }
    if let Some(g) = &plan.graded {
        let cert = if g.certificate.is_empty() {
            String::new()
        } else {
            format!(", certificate {}", g.certificate)
        };
        lines.push(format!("Graded: {} {}{cert}", g.grader, g.grade));
    }
    lines.push("The pictures show the card you will receive.".into());
    lines.join("\n")
}

pub fn preview(item: &ItemSummary) -> Preview {
    let plan = listing::plan(item);
    let (category, condition) = match &plan {
        Ok(p) => {
            let (category, sub) = category(item, p.kind);
            let condition = match graded_condition(item) {
                Some(g) if p.graded.is_none() => {
                    format!("{} (from grading)", condition(p.kind, false, g))
                }
                _ => p.condition_label(),
            };
            (Some(format!("{category} › {sub}")), Some(condition))
        }
        Err(_) => (None, None),
    };
    Preview {
        item_id: item.id,
        seq: item.seq,
        thumb_path: item.thumb_path.clone(),
        title: plan
            .as_ref()
            .map(|p| p.title.clone())
            .unwrap_or_else(|_| item.card.full_name.clone()),
        category,
        condition,
        price: item
            .price
            .filter(|p| p.is_finite() && *p > 0.0)
            .map(|p| whole_dollars(p) as f64),
        ebay: None,
        problem: plan.err(),
    }
}

/// The card's row, with its pictures' URLs.
pub fn row(
    item: &ItemDetail,
    plan: &Listing,
    price: f64,
    options: &Options,
    images: &[String],
) -> Vec<String> {
    let s = &item.summary;
    let (category, sub) = category(s, plan.kind);
    let raw = graded_condition(s).unwrap_or(options.condition);
    let offerable = options.offerable && options.listing_type == ListingType::BuyItNow;
    let mut row = vec![
        category.to_string(),
        sub.to_string(),
        plan.title.clone(),
        description(s, plan),
        "1".into(),
        options.listing_type.as_whatnot().into(),
        whole_dollars(price).to_string(),
        options.shipping_profile.clone(),
        if offerable { "TRUE" } else { "FALSE" }.into(),
        "Not Hazmat".into(),
        condition(plan.kind, plan.graded.is_some(), raw).into(),
        String::new(),
        listing::sku(&s.session_id, &item.event_id),
    ];
    row.extend((0..MAX_IMAGES).map(|i| images.get(i).cloned().unwrap_or_default()));
    row
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Export {
    pub rows: usize,
    pub skipped: Vec<Skipped>,
    /// Rows written without pictures.
    pub without_images: usize,
    /// Why pictures stopped being uploaded, if they did.
    pub image_error: Option<String>,
}

/// Uploads the cards' pictures (unless they are hosted already) and writes the CSV text. `progress` gets
/// (cards done, cards).
pub async fn export(
    runner: &Runner,
    cards: &[PricedItem],
    options: &Options,
    cancel: &CancellationToken,
    progress: impl Fn(usize, usize),
) -> crate::error::Result<(String, Export)> {
    let mut hosting = Hosting::new(runner, options.upload_images)?;
    let mut text = csv_line(&HEADER.iter().map(|h| h.to_string()).collect::<Vec<_>>());
    let mut export = Export::default();
    for (done, priced) in cards.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(ApiError::Cancelled.into());
        }
        let item = runner.db.item(priced.item_id)?;
        match listing::plan(&item.summary) {
            Ok(_) if !(priced.price.is_finite() && priced.price > 0.0) => {
                skip(&mut export, &item, "no price")
            }
            Err(why) => skip(&mut export, &item, &why.replace("on eBay ", "on Whatnot ")),
            Ok(plan) => {
                let images = hosting.urls(runner, &item, cancel).await?;
                if images.is_empty() {
                    export.without_images += 1;
                }
                text.push_str(&csv_line(&row(
                    &item,
                    &plan,
                    priced.price,
                    options,
                    &images,
                )));
                export.rows += 1;
            }
        }
        progress(done + 1, cards.len());
    }
    export.image_error = hosting.error;
    Ok((text, export))
}

fn skip(export: &mut Export, item: &ItemDetail, reason: &str) {
    export.skipped.push(Skipped {
        seq: item.summary.seq,
        reason: reason.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::test_item as item;
    use crate::model::SlabLabel;

    fn options() -> Options {
        Options {
            listing_type: ListingType::BuyItNow,
            shipping_profile: "0-1 oz".into(),
            offerable: true,
            condition: CardCondition::Excellent,
            upload_images: true,
        }
    }

    #[test]
    fn the_header_is_whatnots_template() {
        assert_eq!(
            HEADER.join(","),
            "Category,Sub Category,Title,Description,Quantity,Type,Price,Shipping Profile,Offerable,Hazmat,\
             Condition,Cost Per Item,SKU,Image URL 1,Image URL 2,Image URL 3,Image URL 4,Image URL 5,Image URL 6,\
             Image URL 7,Image URL 8"
        );
    }

    #[test]
    fn a_trading_card_row() {
        let card = item("tcg", "Pokemon");
        let plan = listing::plan(&card.summary).unwrap();
        let row = row(
            &card,
            &plan,
            12.01,
            &options(),
            &["https://i.ibb.co/a.jpg".into()],
        );
        assert_eq!(row.len(), HEADER.len());
        assert_eq!(
            &row[..3],
            [
                "Trading Card Games",
                "Pokémon Cards",
                "2018 Pokemon Guilds of Ravnica Leapfrog #42"
            ]
        );
        assert_eq!(
            &row[4..11],
            [
                "1",
                "Buy it Now",
                "13",
                "0-1 oz",
                "TRUE",
                "Not Hazmat",
                "Light Played"
            ]
        );
        assert_eq!(row[12], "cl-s-e1");
        assert_eq!(row[13], "https://i.ibb.co/a.jpg");
        assert!(row[3].contains("Number: 42"));
    }

    #[test]
    fn slabs_are_graded_and_sports_raw_cards_use_their_scale() {
        let mut slab = item("sport", "Baseball");
        slab.summary.slab = Some(SlabLabel {
            company: "PSA".into(),
            grade: "10".into(),
            certificate: "87654321".into(),
        });
        let plan = listing::plan(&slab.summary).unwrap();
        let auction = Options {
            listing_type: ListingType::Auction,
            ..options()
        };
        let row = row(&slab, &plan, 1.0, &auction, &[]);
        assert_eq!(&row[..2], ["Sports Cards", "Baseball Singles"]);
        assert_eq!(
            (row[5].as_str(), row[8].as_str(), row[10].as_str()),
            ("Auction", "FALSE", "Graded")
        );
        assert!(row[3].contains("Graded: PSA 10, certificate 87654321"));
        assert_eq!(
            condition(Kind::Sport, false, CardCondition::NearMint),
            "Raw - Near Mint or Better"
        );
        assert_eq!(
            category(&item("tcg", "Garbage Pail Kids").summary, Kind::Tcg),
            ("Entertainment Cards", "Garbage Pail Kids")
        );
        assert_eq!(whole_dollars(0.2), 1);
    }
}
