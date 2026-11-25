//! A card as an eBay listing: category, condition (graded slabs carry eBay's grader/grade descriptors),
//! title, item specifics, and the inventory item and offer the Inventory API takes.
//!
//! Trading cards go to CCG Individual Cards (183454), sports cards to Sports Trading Card Singles
//! (261328), comics to Comics & Graphic Novels (259104). A card is "Graded" only when it is in a slab whose label
//! was read — Ximilar's own grade estimates a condition, it isn't a professional grade.

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::ebay::title;
pub use crate::model::CardCondition;
use crate::model::{EbayListing, ItemSummary, TitleField, TitleTemplate};
use crate::settings::EbaySettings;
use crate::ximilar::recognition::{SPORTS, TCG_GAMES};

/// The Inventory API's bulk calls take at most this many.
pub const MAX_BATCH: usize = 25;
pub const MAX_TITLE: usize = 80;
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
        Some("choose your shipping, payment and return policies in Settings → Selling")
    } else if cfg.location_key.is_empty() {
        Some("choose the location cards ship from in Settings → Selling")
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Tcg,
    Sport,
    /// A single comic issue.
    Comic,
}

impl Kind {
    /// By the card's type as identified, else by its game or sport: a slab's label names neither category.
    pub fn of(item: &ItemSummary) -> Option<Kind> {
        match item.id_type.as_deref() {
            Some("tcg") => return Some(Kind::Tcg),
            Some("sport") => return Some(Kind::Sport),
            Some("comics") => return Some(Kind::Comic),
            _ => {}
        }
        let sub = item.card.subcategory.as_str();
        if TCG_GAMES.iter().any(|g| g.eq_ignore_ascii_case(sub)) {
            Some(Kind::Tcg)
        } else if SPORTS.iter().any(|s| s.eq_ignore_ascii_case(sub)) {
            Some(Kind::Sport)
        } else if sub.eq_ignore_ascii_case("comics") {
            Some(Kind::Comic)
        } else {
            None
        }
    }

    /// EBAY_US's category a listing goes in.
    pub fn category(self) -> &'static str {
        match self {
            Kind::Tcg => "183454",
            Kind::Sport => "261328",
            Kind::Comic => "259104",
        }
    }

    /// EBAY_US's category a price search looks in: comics in all of Comic Books & Memorabilia.
    pub fn search_category(self) -> &'static str {
        match self {
            Kind::Comic => "63",
            kind => kind.category(),
        }
    }

    pub fn category_name(self) -> &'static str {
        match self {
            Kind::Tcg => "CCG Individual Cards",
            Kind::Sport => "Sports Trading Card Singles",
            Kind::Comic => "Comics & Graphic Novels",
        }
    }

    /// Descriptor 40001 (Card Condition): the value ids differ between the two card categories; comics have none.
    fn condition_value(self, condition: CardCondition) -> Option<&'static str> {
        Some(match (self, condition) {
            (Kind::Comic, _) => return None,
            (_, CardCondition::NearMint) => "400010",
            (Kind::Tcg, CardCondition::Excellent) => "400015",
            (Kind::Tcg, CardCondition::VeryGood) => "400016",
            (Kind::Tcg, CardCondition::Poor) => "400017",
            (Kind::Sport, CardCondition::Excellent) => "400011",
            (Kind::Sport, CardCondition::VeryGood) => "400012",
            (Kind::Sport, CardCondition::Poor) => "400013",
        })
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
    /// The title is the card's own (`ItemSummary::title`), not a template's.
    pub custom: bool,
    /// The title its template makes, whether or not the card has its own.
    pub automatic: String,
    /// The template that made `automatic`; empty = the built-in one.
    pub template: String,
    /// Parts the card has that didn't fit into `automatic`.
    pub left_out: Vec<TitleField>,
}

impl Listing {
    pub fn condition_label(&self) -> String {
        match &self.graded {
            Some(g) => format!("Graded · {} {}", g.grader, g.grade),
            None => "Ungraded".into(),
        }
    }
}

/// How the card would be listed, or why it can't be. `titles`: the user's title templates
/// (`Settings::titles`).
pub fn plan(item: &ItemSummary, titles: &[TitleTemplate]) -> Result<Listing, String> {
    if item.card.name.trim().is_empty() {
        return Err("the card isn't identified".into());
    }
    let kind = kind(item)?;
    let graded = graded(item, kind)?;
    let made = title::compose(&item.card, kind, graded.as_ref(), titles, MAX_TITLE);
    let own = clip(&item.title, MAX_TITLE);
    Ok(Listing {
        kind,
        graded,
        custom: !own.is_empty(),
        title: if own.is_empty() {
            made.text.clone()
        } else {
            own
        },
        automatic: made.text,
        template: made.template,
        left_out: made.left_out,
    })
}

pub fn kind(item: &ItemSummary) -> Result<Kind, String> {
    Kind::of(item).ok_or_else(|| {
        "unknown card type (only trading cards, sports cards and comics are listed)".into()
    })
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
    let grader = if company.is_empty() {
        "Graded".to_string()
    } else {
        company.clone()
    };
    let certificate: String = slab.certificate.trim().chars().take(MAX_CERT).collect();
    // A comic's grade goes in item specifics, not descriptors, and keeps its tenths: CGC 9.8 is no 10.
    if kind == Kind::Comic {
        let Some(grade) = comic_grade(&slab.grade) else {
            return Err("the slab's grade couldn't be read".into());
        };
        return Ok(Some(Graded {
            grader,
            grader_id: "",
            grade: format_grade(grade),
            grade_id: String::new(),
            certificate,
        }));
    }
    let only_elsewhere = match kind {
        Kind::Tcg => SPORTS_ONLY_GRADERS.contains(&company.as_str()),
        Kind::Sport => TCG_ONLY_GRADERS.contains(&company.as_str()),
        Kind::Comic => false,
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
        grader,
        grader_id,
        grade,
        grade_id,
        certificate,
    }))
}

/// The last number on a slab's grade ("GEM MT 10" → 10).
fn last_number(text: &str) -> Option<f64> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter_map(|part| part.trim_matches('.').parse::<f64>().ok())
        .next_back()
}

/// A card's grade to the nearest half, within 1–10.
fn grade_number(text: &str) -> Option<f64> {
    let grade = (last_number(text)? * 2.0).round() / 2.0;
    (1.0..=10.0).contains(&grade).then_some(grade)
}

/// A comic's grade to the tenth, within 0.5–10 (CGC 9.8, 1.8).
fn comic_grade(text: &str) -> Option<f64> {
    let grade = (last_number(text)? * 10.0).round() / 10.0;
    (0.5..=10.0).contains(&grade).then_some(grade)
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
pub(crate) fn title_game(game: &str) -> &str {
    match game {
        "Magic The Gathering" => "MTG",
        "YuGiOh!" => "Yu-Gi-Oh!",
        "Lotr" => "LOTR",
        other => other,
    }
}

pub(crate) fn clip_words(text: &str, limit: usize) -> String {
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

/// Item specifics. A card's grade and certification go in condition descriptors, not here (eBay retired those);
/// a comic's stay here, as comics have no descriptors.
pub fn aspects(item: &ItemSummary, listing: &Listing) -> Map<String, Value> {
    let card = &item.card;
    let pairs: Vec<(&str, String)> = match listing.kind {
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
        Kind::Comic => {
            let year = year_of(&card.year);
            let mut pairs = vec![
                ("Series Title", card.name.clone()),
                ("Issue Number", card.card_number.clone()),
                ("Publisher", card.set_name.clone()),
                ("Publication Year", year.clone()),
                ("Era", era(&year).to_string()),
            ];
            if let Some(g) = &listing.graded {
                pairs.extend([
                    ("Professional Grader", comic_grader(&g.grader)),
                    ("Grade", comic_grade_name(&g.grade)),
                    ("Certification Number", g.certificate.clone()),
                ]);
            }
            pairs
        }
    };
    pairs
        .into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k.to_string(), json!([clip(&v, MAX_ASPECT)])))
        .collect()
}

/// The four digits of a year in a comic's cover date ("August 1976"), else the date as it is.
pub(crate) fn year_of(date: &str) -> String {
    date.split(|c: char| !c.is_ascii_digit())
        .find(|d| d.len() == 4)
        .map(str::to_string)
        .unwrap_or_else(|| date.trim().to_string())
}

/// eBay's "Era" of a comic by its year.
fn era(year: &str) -> &'static str {
    match year.parse::<u32>() {
        Ok(y) if y < 1938 => "",
        Ok(y) if y <= 1955 => "Golden Age (1938-55)",
        Ok(y) if y <= 1969 => "Silver Age (1956-69)",
        Ok(y) if y <= 1983 => "Bronze Age (1970-83)",
        Ok(y) if y <= 1991 => "Copper Age (1984-91)",
        Ok(_) => "Modern Age (1992-Now)",
        Err(_) => "",
    }
}

/// How eBay names a comic grader.
fn comic_grader(grader: &str) -> String {
    match grader {
        "CGC" => "Certified Guaranty Company (CGC)",
        "CBCS" => "Comic Book Certification Service (CBCS)",
        "PGX" => "Professional Grading Experts (PGX)",
        other => other,
    }
    .to_string()
}

/// A comic's grade with its name on the 10-point scale: "9.8 Near Mint/Mint".
fn comic_grade_name(grade: &str) -> String {
    const NAMES: &[(f64, &str)] = &[
        (10.0, "Gem Mint"),
        (9.9, "Mint"),
        (9.8, "Near Mint/Mint"),
        (9.6, "Near Mint+"),
        (9.4, "Near Mint"),
        (9.2, "Near Mint-"),
        (9.0, "Very Fine/Near Mint"),
        (8.5, "Very Fine+"),
        (8.0, "Very Fine"),
        (7.5, "Very Fine-"),
        (7.0, "Fine/Very Fine"),
        (6.5, "Fine+"),
        (6.0, "Fine"),
        (5.5, "Fine-"),
        (5.0, "Very Good/Fine"),
        (4.5, "Very Good+"),
        (4.0, "Very Good"),
        (3.5, "Very Good-"),
        (3.0, "Good/Very Good"),
        (2.5, "Good+"),
        (2.0, "Good"),
        (1.8, "Good-"),
        (1.5, "Fair/Good"),
        (1.0, "Fair"),
        (0.5, "Poor"),
    ];
    let Ok(number) = grade.parse::<f64>() else {
        return grade.to_string();
    };
    match NAMES.iter().find(|(n, _)| (n - number).abs() < 0.05) {
        Some((n, name)) => format!("{n:.1} {name}"),
        None => format!("{number:.1}"),
    }
}

/// eBay's condition and its descriptors (none for a comic).
pub fn condition(listing: &Listing, ungraded: CardCondition) -> (&'static str, Value) {
    if listing.kind == Kind::Comic {
        let condition = match &listing.graded {
            Some(g) => match g.grade.parse::<f64>().unwrap_or(0.0) {
                g if g >= 9.0 => "LIKE_NEW",
                g if g >= 6.0 => "USED_VERY_GOOD",
                g if g >= 3.0 => "USED_GOOD",
                _ => "USED_ACCEPTABLE",
            },
            None => match ungraded {
                CardCondition::NearMint => "LIKE_NEW",
                CardCondition::Excellent => "USED_VERY_GOOD",
                CardCondition::VeryGood => "USED_GOOD",
                CardCondition::Poor => "USED_ACCEPTABLE",
            },
        };
        return (condition, json!([]));
    }
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
            json!([{"name": "40001", "values": [listing.kind.condition_value(ungraded).unwrap_or_default()]}]),
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
    let comic = listing.kind == Kind::Comic;
    let labels = if comic {
        ["Publisher", "Issue", "Cover date"]
    } else {
        ["Set", "Number", "Year"]
    };
    let mut facts = vec![];
    for (label, value) in labels
        .into_iter()
        .zip([&card.set_name, &card.card_number, &card.year])
    {
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
        "<p><b>{}</b></p><ul>{}</ul><p>The pictures show the {} you will receive.</p>",
        escape(&listing.title),
        facts.join(""),
        if comic { "comic" } else { "card" }
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
            "aspects": aspects(item, listing),
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
    /// The title is the card's own, not a template's.
    pub custom_title: bool,
    /// Parts the card has that didn't fit into the title.
    pub left_out: Vec<TitleField>,
    pub category: Option<String>,
    pub condition: Option<String>,
    /// The price it is offered at, else Ximilar's price.
    pub price: Option<f64>,
    pub ebay: Option<EbayListing>,
    /// Why it can't be listed.
    pub problem: Option<String>,
}

impl Preview {
    /// Whether the planned title is the card's own, and what didn't fit into it.
    pub fn titled(plan: &Result<Listing, String>) -> (bool, Vec<TitleField>) {
        match plan {
            Ok(p) if !p.custom => (false, p.left_out.clone()),
            Ok(_) => (true, Vec::new()),
            Err(_) => (false, Vec::new()),
        }
    }
}

pub fn preview(item: &ItemSummary, titles: &[TitleTemplate]) -> Preview {
    let planned = plan(item, titles);
    let (custom_title, left_out) = Preview::titled(&planned);
    let price = item
        .ebay
        .as_ref()
        .and_then(|e| e.price)
        .or(item.usd_price())
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
        custom_title,
        left_out,
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

    /// Without templates of the user's, as most of these cards are listed.
    fn plan(item: &ItemSummary) -> Result<Listing, String> {
        super::plan(item, &[])
    }

    fn preview(item: &ItemSummary) -> Preview {
        super::preview(item, &[])
    }

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
            price_currency: "USD".into(),
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
            title: String::new(),
            stage: None,
            ebay: None,
            market_range: None,
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
        let aspects = aspects(&card("sport", "Baseball", None), &sport);
        assert_eq!(aspects["Player/Athlete"], json!(["Charizard ex"]));
        assert_eq!(aspects["Sport"], json!(["Baseball"]));
        let aspects = super::aspects(&card("tcg", "Pokemon", None), &tcg);
        assert_eq!(aspects["Game"], json!(["Pokémon TCG"]));
    }

    #[test]
    fn slab_cards_are_typed_by_their_game_and_comics_by_their_type() {
        assert_eq!(
            plan(&card("slab", "Basketball", None)).unwrap().kind,
            Kind::Sport
        );
        assert_eq!(
            plan(&card("slab", "YuGiOh!", None)).unwrap().kind,
            Kind::Tcg
        );
        assert_eq!(
            plan(&card("slab", "Comics", None)).unwrap().kind,
            Kind::Comic
        );
        assert!(plan(&card("slab", "", None)).is_err());
        assert_eq!(plan(&card("comics", "", None)).unwrap().kind, Kind::Comic);
        let mut unknown = card("tcg", "Pokemon", None);
        unknown.card.name.clear();
        assert!(plan(&unknown).is_err());
    }

    fn comic(slab: Option<(&str, &str, &str)>) -> ItemSummary {
        let mut item = card("comics", "Comics", slab);
        item.card = CardFields {
            name: "Amazing Spider-Man".into(),
            set_name: "Marvel".into(),
            card_number: "300".into(),
            year: "May 1988".into(),
            series: "1988-05".into(),
            subcategory: "Comics".into(),
            ..Default::default()
        };
        item
    }

    #[test]
    fn comics_list_as_single_issues() {
        let raw = plan(&comic(None)).unwrap();
        assert_eq!(raw.title, "Amazing Spider-Man #300 1988 Marvel");
        assert_eq!(
            (raw.kind.category(), raw.kind.search_category()),
            ("259104", "63")
        );
        assert_eq!(
            condition(&raw, CardCondition::VeryGood),
            ("USED_GOOD", json!([]))
        );
        let aspects = aspects(&comic(None), &raw);
        assert_eq!(aspects["Series Title"], json!(["Amazing Spider-Man"]));
        assert_eq!(aspects["Issue Number"], json!(["300"]));
        assert_eq!(aspects["Publisher"], json!(["Marvel"]));
        assert_eq!(aspects["Publication Year"], json!(["1988"]));
        assert_eq!(aspects["Era"], json!(["Copper Age (1984-91)"]));
        assert!(!aspects.contains_key("Grade"));
        assert!(description(&comic(None), &raw).contains("the comic you will receive"));

        // CGC grades in tenths; the grade goes in item specifics, as comics have no descriptors.
        let slab = comic(Some(("CGC", "NM/M 9.8", "1234567001")));
        let graded = plan(&slab).unwrap();
        assert_eq!(graded.title, "Amazing Spider-Man #300 1988 Marvel CGC 9.8");
        assert_eq!(graded.condition_label(), "Graded · CGC 9.8");
        assert_eq!(
            condition(&graded, CardCondition::Poor),
            ("LIKE_NEW", json!([]))
        );
        let aspects = super::aspects(&slab, &graded);
        assert_eq!(
            aspects["Professional Grader"],
            json!(["Certified Guaranty Company (CGC)"])
        );
        assert_eq!(aspects["Grade"], json!(["9.8 Near Mint/Mint"]));
        assert_eq!(aspects["Certification Number"], json!(["1234567001"]));
        let low = plan(&comic(Some(("CBCS", "1.8", "")))).unwrap();
        assert_eq!(low.graded.unwrap().grade, "1.8");
        assert_eq!(
            offer(&slab, &graded, "cl-1", &EbaySettings::default(), 300.0)["categoryId"],
            "259104"
        );
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
        // What still fits comes back, instead of every part that was tried before it going too.
        item.card.set_name += " that goes on and on";
        assert_eq!(title(&item), "2023 MTG Jace, the Mind Sculptor #125 BGS 9");
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
