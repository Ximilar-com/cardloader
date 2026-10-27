//! A card as an eBay listing: category, condition (graded slabs carry eBay's grader/grade descriptors),
//! title, item specifics, and the inventory item and offer the Inventory API takes.
//!
//! Trading cards go to CCG Individual Cards (183454), sports cards to Sports Trading Card Singles
//! (261328); comics aren't listed yet. A card is "Graded" only when it is in a slab whose label was read —
//! Ximilar's own grade estimates a condition, it isn't a professional grade.

use serde::Serialize;
use serde_json::{json, Map, Value};

pub use crate::model::CardCondition;
use crate::model::{EbayListing, ItemSummary};
use crate::settings::EbaySettings;
use crate::ximilar::recognition::{SPORTS, TCG_GAMES};

/// The Inventory API's bulk calls take at most this many.
pub const MAX_BATCH: usize = 25;
const MAX_TITLE: usize = 80;
const MAX_ASPECT: usize = 65;
const MAX_SKU: usize = 50;
const MAX_CERT: usize = 30;

pub struct Marketplace {
    pub id: &'static str,
    pub currency: &'static str,
    /// The inventory item's `locale`.
    pub locale: &'static str,
    pub language: &'static str,
}

/// The marketplaces cardloader lists on; the category and condition ids below are EBAY_US's.
const MARKETPLACES: &[Marketplace] = &[Marketplace {
    id: "EBAY_US",
    currency: "USD",
    locale: "en_US",
    language: "en-US",
}];

impl Marketplace {
    pub fn of(id: &str) -> &'static Marketplace {
        MARKETPLACES
            .iter()
            .find(|m| m.id == id)
            .unwrap_or(&MARKETPLACES[0])
    }
}

/// What a listing setup still lacks, if anything.
pub fn setup_missing(cfg: &EbaySettings) -> Option<&'static str> {
    if cfg.fulfillment_policy_id.is_empty()
        || cfg.payment_policy_id.is_empty()
        || cfg.return_policy_id.is_empty()
    {
        Some("choose your shipping, payment and return policies in Settings › eBay")
    } else if cfg.location_key.is_empty() {
        Some("choose the location cards ship from in Settings › eBay")
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Tcg,
    Sport,
}

impl Kind {
    pub fn category(self) -> &'static str {
        match self {
            Kind::Tcg => "183454",
            Kind::Sport => "261328",
        }
    }

    pub fn category_name(self) -> &'static str {
        match self {
            Kind::Tcg => "CCG Individual Cards",
            Kind::Sport => "Sports Trading Card Singles",
        }
    }

    /// Descriptor 40001 (Card Condition): the value ids differ between the two categories.
    fn condition_value(self, condition: CardCondition) -> &'static str {
        match (self, condition) {
            (_, CardCondition::NearMint) => "400010",
            (Kind::Tcg, CardCondition::Excellent) => "400015",
            (Kind::Tcg, CardCondition::VeryGood) => "400016",
            (Kind::Tcg, CardCondition::Poor) => "400017",
            (Kind::Sport, CardCondition::Excellent) => "400011",
            (Kind::Sport, CardCondition::VeryGood) => "400012",
            (Kind::Sport, CardCondition::Poor) => "400013",
        }
    }
}

/// Descriptor 27501 (Professional Grader) values; some graders exist in one category only.
const GRADERS: &[(&str, &str)] = &[
    ("PSA", "275010"),
    ("BCCG", "275011"),
    ("BVG", "275012"),
    ("BGS", "275013"),
    ("CSG", "275014"),
    ("CGC", "275015"),
    ("SGC", "275016"),
    ("KSA", "275017"),
    ("GMA", "275018"),
    ("HGA", "275019"),
    ("ISA", "2750110"),
    ("PCA", "2750111"),
    ("GSG", "2750112"),
    ("PGS", "2750113"),
    ("MNT", "2750114"),
    ("TAG", "2750115"),
    ("RARE", "2750116"),
    ("RCG", "2750117"),
    ("PCG", "2750118"),
    ("ACE", "2750119"),
    ("CGA", "2750120"),
    ("TCG", "2750121"),
    ("ARK", "2750122"),
];
const OTHER_GRADER: &str = "2750123";
const SPORTS_ONLY_GRADERS: &[&str] = &["CSG"];
const TCG_ONLY_GRADERS: &[&str] = &["PCA", "PCG", "ACE", "TCG", "ARK"];
const AUTHENTIC_GRADE: &str = "2750219";

/// A slabbed card's grading, as eBay's condition descriptors.
#[derive(Debug, Clone, PartialEq)]
pub struct Graded {
    pub grader: String,
    pub grader_id: &'static str,
    /// "9.5", or "Authentic".
    pub grade: String,
    pub grade_id: String,
    pub certificate: String,
}

/// What a card would be listed as.
#[derive(Debug, Clone, PartialEq)]
pub struct Listing {
    pub kind: Kind,
    pub graded: Option<Graded>,
    pub title: String,
}

impl Listing {
    pub fn condition_label(&self) -> String {
        match &self.graded {
            Some(g) => format!("Graded · {} {}", g.grader, g.grade),
            None => "Ungraded".into(),
        }
    }
}

/// How the card would be listed, or why it can't be.
pub fn plan(item: &ItemSummary) -> Result<Listing, String> {
    if item.card.name.trim().is_empty() {
        return Err("the card isn't identified".into());
    }
    let kind = kind(item)?;
    let graded = graded(item, kind)?;
    let title = title(item, kind, graded.as_ref());
    Ok(Listing {
        kind,
        graded,
        title,
    })
}

pub fn kind(item: &ItemSummary) -> Result<Kind, String> {
    match item.id_type.as_deref() {
        Some("tcg") => return Ok(Kind::Tcg),
        Some("sport") => return Ok(Kind::Sport),
        Some("comics") => return Err("comics can't be listed on eBay yet".into()),
        _ => {}
    }
    let sub = item.card.subcategory.as_str();
    if TCG_GAMES.iter().any(|g| g.eq_ignore_ascii_case(sub)) {
        Ok(Kind::Tcg)
    } else if SPORTS.iter().any(|s| s.eq_ignore_ascii_case(sub)) {
        Ok(Kind::Sport)
    } else {
        Err("unknown card type (only trading and sports cards are listed)".into())
    }
}

fn graded(item: &ItemSummary, kind: Kind) -> Result<Option<Graded>, String> {
    let Some(slab) = item
        .slab
        .as_ref()
        .filter(|s| !s.company.trim().is_empty() || !s.grade.trim().is_empty())
    else {
        return Ok(None);
    };
    let company: String = slab
        .company
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_uppercase();
    let company = if company == "BECKETT" {
        "BGS".to_string()
    } else {
        company
    };
    let only_elsewhere = match kind {
        Kind::Tcg => SPORTS_ONLY_GRADERS.contains(&company.as_str()),
        Kind::Sport => TCG_ONLY_GRADERS.contains(&company.as_str()),
    };
    let grader_id = GRADERS
        .iter()
        .find(|(name, _)| *name == company && !only_elsewhere)
        .map(|(_, id)| *id)
        .unwrap_or(OTHER_GRADER);
    let (grade, grade_id) = match grade_number(&slab.grade) {
        Some(g) => (
            format_grade(g),
            // 10 → 275020, 9.5 → 275021 … 1 → 2750218.
            format!("27502{}", ((10.0 - g) * 2.0).round() as u32),
        ),
        None if slab.grade.to_ascii_lowercase().contains("auth") => {
            ("Authentic".to_string(), AUTHENTIC_GRADE.to_string())
        }
        None => return Err("the slab's grade couldn't be read".into()),
    };
    Ok(Some(Graded {
        grader: if company.is_empty() {
            "Graded".into()
        } else {
            company
        },
        grader_id,
        grade,
        grade_id,
        certificate: slab.certificate.trim().chars().take(MAX_CERT).collect(),
    }))
}

/// The last number on a slab's grade ("GEM MT 10" → 10), to the nearest half, within 1–10.
fn grade_number(text: &str) -> Option<f64> {
    let grade = text
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter_map(|part| part.trim_matches('.').parse::<f64>().ok())
        .next_back()?;
    let grade = (grade * 2.0).round() / 2.0;
    (1.0..=10.0).contains(&grade).then_some(grade)
}

fn format_grade(grade: f64) -> String {
    if grade.fract() == 0.0 {
        format!("{grade:.0}")
    } else {
        format!("{grade:.1}")
    }
}

/// How eBay names a game in the "Game" item specific.
fn ebay_game(game: &str) -> String {
    match game {
        "Pokemon" => "Pokémon TCG",
        "Magic The Gathering" => "Magic: The Gathering",
        "YuGiOh!" => "Yu-Gi-Oh! TCG",
        "One Piece" => "One Piece Card Game",
        "Lorcana" => "Disney Lorcana",
        "Lotr" => "The Lord of the Rings",
        other => other,
    }
    .to_string()
}

/// How sellers write a game in titles.
fn title_game(game: &str) -> &str {
    match game {
        "Magic The Gathering" => "MTG",
        "YuGiOh!" => "Yu-Gi-Oh!",
        "Lotr" => "LOTR",
        other => other,
    }
}

/// `2023 Pokemon Obsidian Flames Charizard ex #125 PSA 10`, without the game, year and set (in that order)
/// when it is too long.
fn title(item: &ItemSummary, kind: Kind, graded: Option<&Graded>) -> String {
    let card = &item.card;
    let game = match kind {
        Kind::Tcg => title_game(&card.subcategory).to_string(),
        Kind::Sport => String::new(),
    };
    let number =
        (!card.card_number.trim().is_empty()).then(|| format!("#{}", card.card_number.trim()));
    let grade = graded.map(|g| format!("{} {}", g.grader, g.grade));
    let parts = |with_game: bool, with_year: bool, with_set: bool| -> String {
        [
            with_year.then(|| card.year.clone()),
            with_game.then(|| game.clone()),
            with_set.then(|| card.set_name.clone()),
            Some(card.name.clone()),
            number.clone(),
            grade.clone(),
        ]
        .into_iter()
        .flatten()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
    };
    for (with_game, year, set) in [
        (true, true, true),
        (false, true, true),
        (false, false, true),
        (false, false, false),
    ] {
        let text = parts(with_game, year, set);
        if text.chars().count() <= MAX_TITLE {
            return text;
        }
    }
    clip_words(&parts(false, false, false), MAX_TITLE)
}

fn clip_words(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out = String::new();
    for word in text.split_whitespace() {
        let next = if out.is_empty() {
            word.to_string()
        } else {
            format!("{out} {word}")
        };
        if next.chars().count() > limit {
            break;
        }
        out = next;
    }
    if out.is_empty() {
        text.chars().take(limit).collect()
    } else {
        out
    }
}

fn clip(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}

/// Item specifics. Grade and certification go in condition descriptors, not here (eBay retired those).
pub fn aspects(item: &ItemSummary, kind: Kind) -> Map<String, Value> {
    let card = &item.card;
    let pairs: Vec<(&str, String)> = match kind {
        Kind::Tcg => vec![
            ("Game", ebay_game(&card.subcategory)),
            ("Card Name", card.name.clone()),
            ("Set", card.set_name.clone()),
            ("Card Number", card.card_number.clone()),
            ("Year Manufactured", card.year.clone()),
        ],
        Kind::Sport => vec![
            ("Sport", card.subcategory.clone()),
            ("Player/Athlete", card.name.clone()),
            ("Set", card.set_name.clone()),
            ("Card Number", card.card_number.clone()),
            ("Year Manufactured", card.year.clone()),
        ],
    };
    pairs
        .into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k.to_string(), json!([clip(&v, MAX_ASPECT)])))
        .collect()
}

/// eBay's condition and its descriptors.
pub fn condition(listing: &Listing, ungraded: CardCondition) -> (&'static str, Value) {
    match &listing.graded {
        Some(g) => {
            let mut descriptors = vec![
                json!({"name": "27501", "values": [g.grader_id]}),
                json!({"name": "27502", "values": [g.grade_id]}),
            ];
            if !g.certificate.is_empty() {
                descriptors.push(json!({"name": "27503", "additionalInfo": g.certificate}));
            }
            ("LIKE_NEW", Value::Array(descriptors))
        }
        None => (
            "USED_VERY_GOOD",
            json!([{"name": "40001", "values": [listing.kind.condition_value(ungraded)]}]),
        ),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn description(item: &ItemSummary, listing: &Listing) -> String {
    let card = &item.card;
    let mut facts = vec![];
    for (label, value) in [
        ("Set", &card.set_name),
        ("Number", &card.card_number),
        ("Year", &card.year),
    ] {
        if !value.trim().is_empty() {
            facts.push(format!("<li>{label}: {}</li>", escape(value.trim())));
        }
    }
    if let Some(g) = &listing.graded {
        let cert = if g.certificate.is_empty() {
            String::new()
        } else {
            format!(", certificate {}", escape(&g.certificate))
        };
        facts.push(format!(
            "<li>Graded: {} {}{cert}</li>",
            escape(&g.grader),
            escape(&g.grade)
        ));
    }
    format!(
        "<p><b>{}</b></p><ul>{}</ul><p>The pictures show the card you will receive.</p>",
        escape(&listing.title),
        facts.join("")
    )
}

/// The card's SKU: the session's and the card's ids (event ids repeat across sessions).
pub fn sku(session_id: &str, event_id: &str) -> String {
    let alnum = |s: &str| -> String { s.chars().filter(char::is_ascii_alphanumeric).collect() };
    let session: String = alnum(session_id).chars().take(8).collect();
    format!("cl-{session}-{}", alnum(event_id))
        .chars()
        .take(MAX_SKU)
        .collect()
}

/// A `bulk_create_or_replace_inventory_item` request entry.
pub fn inventory_item(
    item: &ItemSummary,
    listing: &Listing,
    sku: &str,
    market: &Marketplace,
    ungraded: CardCondition,
    images: &[String],
) -> Value {
    let (condition, descriptors) = condition(listing, ungraded);
    json!({
        "sku": sku,
        "locale": market.locale,
        "product": {
            "title": listing.title,
            "description": description(item, listing),
            "aspects": aspects(item, listing.kind),
            "imageUrls": images,
        },
        "condition": condition,
        "conditionDescriptors": descriptors,
        "availability": {"shipToLocationAvailability": {"quantity": 1}},
    })
}

/// An offer for one card at `price`, fixed price until it sells.
pub fn offer(
    item: &ItemSummary,
    listing: &Listing,
    sku: &str,
    cfg: &EbaySettings,
    price: f64,
) -> Value {
    let market = Marketplace::of(&cfg.marketplace);
    json!({
        "sku": sku,
        "marketplaceId": market.id,
        "format": "FIXED_PRICE",
        "availableQuantity": 1,
        "categoryId": listing.kind.category(),
        "listingDescription": description(item, listing),
        "listingDuration": "GTC",
        "listingPolicies": {
            "fulfillmentPolicyId": cfg.fulfillment_policy_id,
            "paymentPolicyId": cfg.payment_policy_id,
            "returnPolicyId": cfg.return_policy_id,
        },
        "merchantLocationKey": cfg.location_key,
        "pricingSummary": {"price": {"value": format!("{price:.2}"), "currency": market.currency}},
    })
}

/// One card in the "List on eBay" dialog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub item_id: i64,
    pub seq: i64,
    pub thumb_path: Option<String>,
    pub title: String,
    pub category: Option<String>,
    pub condition: Option<String>,
    /// The price it is offered at, else Ximilar's price.
    pub price: Option<f64>,
    pub ebay: Option<EbayListing>,
    /// Why it can't be listed.
    pub problem: Option<String>,
}

pub fn preview(item: &ItemSummary) -> Preview {
    let planned = plan(item);
    let price = item
        .ebay
        .as_ref()
        .and_then(|e| e.price)
        .or(item.price)
        .filter(|p| p.is_finite() && *p > 0.0)
        .map(|p| (p * 100.0).round() / 100.0);
    Preview {
        item_id: item.id,
        seq: item.seq,
        thumb_path: item.thumb_path.clone(),
        title: planned
            .as_ref()
            .map(|l| l.title.clone())
            .unwrap_or_else(|_| item.card.full_name.clone()),
        category: planned
            .as_ref()
            .ok()
            .map(|l| l.kind.category_name().to_string()),
        condition: planned.as_ref().ok().map(Listing::condition_label),
        price,
        ebay: item.ebay.clone(),
        problem: planned.err(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CardFields, ItemStatus, SlabLabel};

    fn card(id_type: &str, subcategory: &str, slab: Option<(&str, &str, &str)>) -> ItemSummary {
        ItemSummary {
            id: 1,
            session_id: "s".into(),
            seq: 1,
            status: ItemStatus::Done,
            front_path: "/a.jpg".into(),
            back_path: None,
            thumb_path: None,
            id_type: Some(id_type.into()),
            card: CardFields {
                name: "Charizard ex".into(),
                full_name: "Charizard ex OBF 125".into(),
                set_name: "Obsidian Flames".into(),
                set_code: "OBF".into(),
                card_number: "125".into(),
                year: "2023".into(),
                subcategory: subcategory.into(),
                ..Default::default()
            },
            confidence: Some("high".into()),
            distance: Some(0.2),
            price: Some(12.499),
            grade: Some(7.5),
            condition: None,
            slab: slab.map(|(company, grade, certificate)| SlabLabel {
                company: company.into(),
                grade: grade.into(),
                certificate: certificate.into(),
            }),
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
        }
    }

    #[test]
    fn a_graded_slab_gets_grader_grade_and_certificate() {
        let item = card("tcg", "Pokemon", Some(("PSA", "GEM MT 9.5", "87654321")));
        let listing = plan(&item).unwrap();
        assert_eq!(
            listing.title,
            "2023 Pokemon Obsidian Flames Charizard ex #125 PSA 9.5"
        );
        let (condition, descriptors) = condition(&listing, CardCondition::Poor);
        assert_eq!(condition, "LIKE_NEW");
        assert_eq!(
            descriptors,
            json!([{"name": "27501", "values": ["275010"]}, {"name": "27502", "values": ["275021"]},
                   {"name": "27503", "additionalInfo": "87654321"}])
        );
        assert_eq!(listing.condition_label(), "Graded · PSA 9.5");

        let beckett = plan(&card("sport", "Baseball", Some(("Beckett", "10", "")))).unwrap();
        let g = beckett.graded.unwrap();
        assert_eq!((g.grader_id, g.grade_id.as_str()), ("275013", "275020"));
        let low = plan(&card("tcg", "Pokemon", Some(("CSG", "1", "")))).unwrap();
        let g = low.graded.unwrap();
        assert_eq!(
            (g.grader_id, g.grade_id.as_str()),
            (OTHER_GRADER, "2750218"),
            "CSG grades sports only"
        );
        let auth = plan(&card("tcg", "Pokemon", Some(("PSA", "Authentic", "")))).unwrap();
        assert_eq!(auth.graded.unwrap().grade_id, AUTHENTIC_GRADE);
        assert!(plan(&card("tcg", "Pokemon", Some(("PSA", "", "123")))).is_err());
    }

    #[test]
    fn ungraded_cards_get_their_categorys_condition_values() {
        let tcg = plan(&card("tcg", "Pokemon", None)).unwrap();
        assert_eq!(tcg.kind.category(), "183454");
        assert_eq!(
            condition(&tcg, CardCondition::Excellent),
            (
                "USED_VERY_GOOD",
                json!([{"name": "40001", "values": ["400015"]}])
            )
        );
        let sport = plan(&card("sport", "Baseball", None)).unwrap();
        assert_eq!(sport.kind.category(), "261328");
        assert_eq!(
            condition(&sport, CardCondition::Excellent).1[0]["values"][0],
            "400011"
        );
        assert_eq!(sport.title, "2023 Obsidian Flames Charizard ex #125");
        let aspects = aspects(&card("sport", "Baseball", None), Kind::Sport);
        assert_eq!(aspects["Player/Athlete"], json!(["Charizard ex"]));
        assert_eq!(aspects["Sport"], json!(["Baseball"]));
        let aspects = super::aspects(&card("tcg", "Pokemon", None), Kind::Tcg);
        assert_eq!(aspects["Game"], json!(["Pokémon TCG"]));
    }

    #[test]
    fn slab_cards_are_typed_by_their_game_and_comics_are_not_listed() {
        assert_eq!(
            plan(&card("slab", "Basketball", None)).unwrap().kind,
            Kind::Sport
        );
        assert_eq!(
            plan(&card("slab", "YuGiOh!", None)).unwrap().kind,
            Kind::Tcg
        );
        assert!(plan(&card("slab", "", None)).is_err());
        assert!(plan(&card("comics", "", None))
            .unwrap_err()
            .contains("comics"));
        let mut unknown = card("tcg", "Pokemon", None);
        unknown.card.name.clear();
        assert!(plan(&unknown).is_err());
    }

    #[test]
    fn long_titles_drop_the_least_useful_words_first() {
        let mut item = card("tcg", "Magic The Gathering", Some(("BGS", "9", "")));
        item.card.name = "Jace, the Mind Sculptor".into();
        item.card.set_name = "Worldwake Prerelease Promos".into();
        let title = |item: &ItemSummary| plan(item).unwrap().title;
        assert_eq!(
            title(&item),
            "2023 MTG Worldwake Prerelease Promos Jace, the Mind Sculptor #125 BGS 9"
        );
        item.card.set_name += " and a long name";
        assert_eq!(
            title(&item),
            "Worldwake Prerelease Promos and a long name Jace, the Mind Sculptor #125 BGS 9"
        );
        item.card.set_name += " that goes on and on";
        assert_eq!(title(&item), "Jace, the Mind Sculptor #125 BGS 9");
        item.card.name = "word ".repeat(30);
        assert!(title(&item).chars().count() <= MAX_TITLE);
    }

    #[test]
    fn skus_are_short_and_plain() {
        assert_eq!(
            sku("1b4e28ba-2fa1-41d2-883f-0016d3cca427", "a1b2c3"),
            "cl-1b4e28ba-a1b2c3"
        );
        assert_eq!(sku("s", &"f".repeat(80)).len(), MAX_SKU);
    }

    #[test]
    fn the_offer_uses_the_setup_and_the_price() {
        let item = card("tcg", "Pokemon", None);
        let listing = plan(&item).unwrap();
        let cfg = EbaySettings {
            fulfillment_policy_id: "f1".into(),
            payment_policy_id: "p1".into(),
            return_policy_id: "r1".into(),
            location_key: "home".into(),
            ..Default::default()
        };
        assert_eq!(setup_missing(&cfg), None);
        let offer = offer(&item, &listing, "cl-1", &cfg, 12.5);
        assert_eq!(
            offer["pricingSummary"]["price"],
            json!({"value": "12.50", "currency": "USD"})
        );
        assert_eq!(offer["listingPolicies"]["returnPolicyId"], "r1");
        assert_eq!(offer["merchantLocationKey"], "home");
        assert_eq!(offer["categoryId"], "183454");
        let preview = preview(&item);
        assert_eq!(preview.price, Some(12.5));
        assert_eq!(preview.condition.as_deref(), Some("Ungraded"));
        assert!(setup_missing(&EbaySettings::default()).is_some());
    }
}
