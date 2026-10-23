//! A TCGplayer list of a session's ungraded trading cards: product line, set, name, number, TCGplayer's
//! condition and the price, with the TCGplayer product (from Ximilar's match) to find each card by.
//!
//! It is a list to enter in Seller Portal, not an "Import to Staged" file: that import matches rows by TCGplayer's
//! SKU ids (product + condition + printing + language), which only Seller Portal's own export carries. Graded
//! cards are sold as Listings with Photos and TCGplayer has no sports cards, so neither is listed.

use serde::Serialize;

use super::{csv_line, graded_condition, slabbed, tcgplayer_product, PricedItem, Skipped};
use crate::ebay::listing::{self, Kind, Preview};
use crate::model::{CardCondition, ItemDetail, ItemSummary};

pub const HEADER: &[&str] = &[
    "Product Line",
    "Set Name",
    "Product Name",
    "Number",
    "Condition",
    "Quantity",
    "TCG Marketplace Price",
    "TCGplayer Product Id",
    "Product URL",
];

/// TCGplayer's product line for a game as Ximilar's taxonomy spells it; other games keep their name.
pub fn product_line(game: &str) -> &str {
    match game {
        "Pokemon" => "Pokemon",
        "Magic The Gathering" => "Magic",
        "YuGiOh!" => "YuGiOh",
        "One Piece" => "One Piece Card Game",
        "Lorcana" => "Lorcana TCG",
        "Flesh and Blood" => "Flesh & Blood TCG",
        "Digimon" => "Digimon Card Game",
        "Star Wars Unlimited" => "Star Wars Unlimited",
        "Dragon Ball Super" => "Dragon Ball Super Fusion World",
        "Final Fantasy" => "Final Fantasy TCG",
        "Vanguard" => "Cardfight Vanguard",
        other => other,
    }
}

/// TCGplayer's name for the condition (it has no Mint: Near Mint is its best).
pub fn condition_name(condition: CardCondition) -> &'static str {
    match condition {
        CardCondition::NearMint => "Near Mint",
        CardCondition::Excellent => "Lightly Played",
        CardCondition::VeryGood => "Moderately Played",
        CardCondition::Poor => "Heavily Played",
    }
}

/// Why TCGplayer can't take the card, if it can't.
pub fn problem(item: &ItemSummary) -> Option<String> {
    if item.card.name.trim().is_empty() {
        return Some("the card isn't identified".into());
    }
    match listing::kind(item) {
        Ok(Kind::Tcg) => {}
        Ok(Kind::Sport) => return Some("TCGplayer doesn't sell sports cards".into()),
        Err(why) => return Some(why.replace("eBay", "TCGplayer")),
    }
    slabbed(item)
        .then(|| "graded cards are sold as Listings with Photos on TCGplayer, one by one".into())
}

pub fn preview(item: &ItemSummary) -> Preview {
    let card = &item.card;
    let number = if card.card_number.is_empty() {
        String::new()
    } else {
        format!(" #{}", card.card_number)
    };
    Preview {
        item_id: item.id,
        seq: item.seq,
        thumb_path: item.thumb_path.clone(),
        title: format!("{}{number}", card.name),
        category: Some(
            [product_line(&card.subcategory), card.set_name.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" · "),
        ),
        condition: Some(match graded_condition(item) {
            Some(c) => format!("{} (from grading)", condition_name(c)),
            None => "Ungraded".into(),
        }),
        price: item
            .price
            .filter(|p| p.is_finite() && *p > 0.0)
            .map(|p| ((p * 100.0).round() / 100.0).max(0.01)),
        ebay: None,
        problem: problem(item),
    }
}

/// The card's row; `condition` for cards whose condition wasn't graded.
pub fn row(item: &ItemDetail, price: f64, condition: CardCondition) -> Vec<String> {
    let s = &item.summary;
    let product = tcgplayer_product(item);
    vec![
        product_line(&s.card.subcategory).to_string(),
        s.card.set_name.clone(),
        s.card.name.clone(),
        s.card.card_number.clone(),
        condition_name(graded_condition(s).unwrap_or(condition)).to_string(),
        "1".into(),
        format!("{:.2}", ((price * 100.0).round() / 100.0).max(0.01)),
        product.clone().unwrap_or_default(),
        product
            .map(|id| format!("https://www.tcgplayer.com/product/{id}"))
            .unwrap_or_default(),
    ]
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Export {
    pub rows: usize,
    pub skipped: Vec<Skipped>,
}

/// The list as CSV text: the cards TCGplayer takes, in session order.
pub fn csv(cards: &[(ItemDetail, PricedItem)], condition: CardCondition) -> (String, Export) {
    let mut text = csv_line(&HEADER.iter().map(|h| h.to_string()).collect::<Vec<_>>());
    let mut export = Export {
        rows: 0,
        skipped: Vec::new(),
    };
    for (item, priced) in cards {
        let reason = problem(&item.summary).or_else(|| {
            (!(priced.price.is_finite() && priced.price > 0.0)).then(|| "no price".to_string())
        });
        match reason {
            Some(reason) => export.skipped.push(Skipped {
                seq: item.summary.seq,
                reason,
            }),
            None => {
                text.push_str(&csv_line(&row(item, priced.price, condition)));
                export.rows += 1;
            }
        }
    }
    (text, export)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::test_item as item;
    use crate::model::SlabLabel;

    #[test]
    fn ungraded_trading_cards_become_rows() {
        let mtg = item("tcg", "Magic The Gathering");
        let mut played = item("tcg", "Pokemon");
        played.summary.condition = Some("Lightly Played".into());
        let mut slab = item("tcg", "Pokemon");
        slab.summary.slab = Some(SlabLabel {
            company: "PSA".into(),
            grade: "10".into(),
            certificate: String::new(),
        });
        let sport = item("sport", "Baseball");
        let priced = |id| PricedItem {
            item_id: id,
            price: 0.234,
        };
        let (text, export) = csv(
            &[
                (mtg, priced(1)),
                (played, priced(1)),
                (slab, priced(1)),
                (sport, priced(1)),
            ],
            CardCondition::VeryGood,
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], HEADER.join(","));
        assert_eq!(
            lines[1],
            "Magic,Guilds of Ravnica,Leapfrog,42,Moderately Played,1,0.23,176868,https://www.tcgplayer.com/product/176868"
        );
        assert!(lines[2].starts_with("Pokemon,Guilds of Ravnica,Leapfrog,42,Lightly Played,"));
        assert_eq!(export.rows, 2);
        assert_eq!(export.skipped.len(), 2);
        assert!(export.skipped[0].reason.contains("Listings with Photos"));
        assert!(export.skipped[1].reason.contains("sports"));
    }

    #[test]
    fn the_preview_names_the_product_line() {
        let p = preview(&item("tcg", "YuGiOh!").summary);
        assert_eq!(p.category.as_deref(), Some("YuGiOh · Guilds of Ravnica"));
        assert_eq!(p.price, Some(0.23));
        assert_eq!(p.problem, None);
    }
}
