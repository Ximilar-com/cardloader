//! Shopify's product CSV (Products → Import) and inventory CSV (Products → Inventory → Import).
//!
//! Products: either one product per card (name, set and number), its conditions as variants with the number of
//! copies, or one product per scanned card. A graded slab is always a product of its own (one item, its grader and
//! grade in the title). Ungraded products have a "Condition" option; slabs Shopify's single "Default Title"
//! variant. Handles are made from the card, so importing again with "Overwrite products with matching handles"
//! updates the same products, and the inventory CSV finds them (rows match by handle and option value).
//!
//! Pictures are public URLs Shopify copies during the import (ImgBB, as for Whatnot).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::imgbb::Hosting;
use super::{csv_line, graded_condition, PricedItem, Skipped};
use crate::ebay::listing::{self, Kind, Listing, Preview};
use crate::model::{CardCondition, ItemDetail, ItemSummary};
use crate::runner::Runner;
use crate::ximilar::http::ApiError;

/// The product CSV columns cardloader fills, named as Shopify's 2024 template names them (columns left out of
/// the file stay as they are in the store).
pub const PRODUCT_HEADER: &[&str] = &[
    "Title",
    "URL handle",
    "Description",
    "Vendor",
    "Product category",
    "Type",
    "Tags",
    "Published on online store",
    "Status",
    "SKU",
    "Option1 name",
    "Option1 value",
    "Price",
    "Inventory tracker",
    "Inventory quantity",
    "Continue selling when out of stock",
    "Requires shipping",
    "Product image URL",
    "Image position",
    "Image alt text",
    "Variant image URL",
];

pub const INVENTORY_HEADER: &[&str] = &[
    "Handle",
    "Title",
    "Option1 Name",
    "Option1 Value",
    "Option2 Name",
    "Option2 Value",
    "Option3 Name",
    "Option3 Value",
    "SKU",
    "HS Code",
    "COO",
    "Location",
    "Bin name",
    "Incoming (not editable)",
    "Unavailable (not editable)",
    "Committed (not editable)",
    "Available (not editable)",
    "On hand (current)",
    "On hand (new)",
];

const CATEGORY_ROOT: &str =
    "Arts & Entertainment > Hobbies & Creative Arts > Collectibles > Collectible Trading Cards";
const MAX_HANDLE: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    /// New (or updated) products with their variants.
    #[default]
    Products,
    /// Stock of products imported before.
    Inventory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Grouping {
    /// One product per card; its conditions are variants.
    #[default]
    ByCard,
    /// One product per scanned card.
    PerCard,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub file: FileKind,
    pub grouping: Grouping,
    /// For ungraded cards whose condition wasn't graded.
    pub condition: CardCondition,
    /// Products are active (published) instead of drafts.
    pub active: bool,
    pub upload_images: bool,
    /// The store location the inventory CSV sets stock at, exactly as Shopify names it.
    pub location: String,
}

/// Shopify's category for the card (its product taxonomy).
pub fn category(item: &ItemSummary, kind: Kind) -> String {
    let leaf = match (kind, item.card.subcategory.as_str()) {
        (Kind::Sport, _) => "Sports Trading Cards",
        (Kind::Tcg, "Garbage Pail Kids" | "Marvel Universe") => "Entertainment Cards",
        (Kind::Tcg, _) => "Gaming Cards",
    };
    format!("{CATEGORY_ROOT} > {leaf}")
}

/// The condition as the option value shows it.
pub fn condition_name(kind: Kind, condition: CardCondition) -> &'static str {
    match (kind, condition) {
        (Kind::Tcg, CardCondition::NearMint) => "Near Mint",
        (Kind::Tcg, CardCondition::Excellent) => "Lightly Played",
        (Kind::Tcg, CardCondition::VeryGood) => "Moderately Played",
        (Kind::Tcg, CardCondition::Poor) => "Heavily Played",
        (Kind::Sport, CardCondition::NearMint) => "Near Mint or Better",
        (Kind::Sport, CardCondition::Excellent) => "Excellent",
        (Kind::Sport, CardCondition::VeryGood) => "Very Good",
        (Kind::Sport, CardCondition::Poor) => "Poor",
    }
}

fn condition_code(condition: CardCondition) -> &'static str {
    match condition {
        CardCondition::NearMint => "nm",
        CardCondition::Excellent => "lp",
        CardCondition::VeryGood => "mp",
        CardCondition::Poor => "hp",
    }
}

/// Letters, digits and dashes, lower case; accented letters lose their accent.
pub fn handle(parts: &[&str]) -> String {
    let mut out = String::new();
    for c in parts
        .join(" ")
        .chars()
        .flat_map(char::to_lowercase)
        .map(unaccent)
    {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-')
        .chars()
        .take(MAX_HANDLE)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

fn unaccent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

/// One card going to Shopify: its listing plan, price, condition and picture URLs.
pub struct Card {
    pub item: ItemDetail,
    pub plan: Listing,
    pub price: f64,
    pub condition: CardCondition,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    /// `None`: the product's only variant ("Default Title").
    pub condition: Option<&'static str>,
    pub sku: String,
    /// The highest price among its copies.
    pub price: f64,
    pub quantity: usize,
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    pub handle: String,
    pub title: String,
    pub description: String,
    pub vendor: String,
    pub category: String,
    pub product_type: &'static str,
    pub tags: Vec<String>,
    pub variants: Vec<Variant>,
    pub images: Vec<String>,
}

/// The cards as products, in the order the cards come.
pub fn products(cards: &[Card], grouping: Grouping) -> Vec<Product> {
    let mut products: Vec<Product> = Vec::new();
    let mut by_key: BTreeMap<String, usize> = BTreeMap::new();
    for card in cards {
        let s = &card.item.summary;
        let c = &s.card;
        let graded = card.plan.graded.is_some();
        let own = graded || grouping == Grouping::PerCard;
        // Ungraded copies of a card share a product (and its handle) across sessions.
        let base = handle(&[&c.subcategory, &c.set_name, &c.name, &c.card_number]);
        let handle = match &card.plan.graded {
            Some(g) => {
                let id = if g.certificate.is_empty() {
                    card.item.event_id.as_str()
                } else {
                    g.certificate.as_str()
                };
                self::handle(&[&base, &g.grader, &g.grade, id])
            }
            None if own => self::handle(&[&base, &card.item.event_id]),
            None => base.clone(),
        };
        let condition = (!graded).then(|| condition_name(card.plan.kind, card.condition));
        let sku = if own {
            listing::sku(&s.session_id, &card.item.event_id)
        } else {
            format!("{base}-{}", condition_code(card.condition))
        };
        let at = match by_key.get(&handle) {
            Some(&at) if !own => at,
            _ => {
                // An ungraded card's title has no grade: every copy of the card fits it.
                let title = card.plan.title.clone();
                let mut tags: Vec<String> =
                    [c.subcategory.as_str(), c.set_name.as_str(), c.year.as_str()]
                        .iter()
                        .filter(|t| !t.trim().is_empty())
                        .map(|t| t.trim().replace(',', " "))
                        .collect();
                if let Some(g) = &card.plan.graded {
                    tags.push("Graded".into());
                    tags.push(g.grader.clone());
                    tags.push(format!("{} {}", g.grader, g.grade));
                }
                products.push(Product {
                    handle: handle.clone(),
                    title,
                    description: listing::description(s, &card.plan),
                    vendor: c.subcategory.clone(),
                    category: category(s, card.plan.kind),
                    product_type: match (graded, card.plan.kind) {
                        (true, _) => "Graded Card",
                        (false, Kind::Sport) => "Sports Card",
                        (false, Kind::Tcg) => "Trading Card",
                    },
                    tags,
                    variants: Vec::new(),
                    images: Vec::new(),
                });
                by_key.insert(handle.clone(), products.len() - 1);
                products.len() - 1
            }
        };
        let product = &mut products[at];
        for url in &card.images {
            if !product.images.contains(url) {
                product.images.push(url.clone());
            }
        }
        match product
            .variants
            .iter_mut()
            .find(|v| v.condition == condition && !own)
        {
            Some(v) => {
                v.quantity += 1;
                v.price = v.price.max(card.price);
                if v.image.is_none() {
                    v.image = card.images.first().cloned();
                }
            }
            None => product.variants.push(Variant {
                condition,
                sku,
                price: card.price,
                quantity: 1,
                image: card.images.first().cloned(),
            }),
        }
    }
    products
}

fn option(v: &Variant) -> (&'static str, &'static str) {
    match v.condition {
        Some(c) => ("Condition", c),
        None => ("Title", "Default Title"),
    }
}

/// The product CSV: a product's first row carries the product, its first variant and first picture; then one row
/// per further variant, then one per further picture.
pub fn product_csv(products: &[Product], active: bool) -> String {
    let mut text = csv_line(
        &PRODUCT_HEADER
            .iter()
            .map(|h| h.to_string())
            .collect::<Vec<_>>(),
    );
    let width = PRODUCT_HEADER.len();
    for p in products {
        for (i, v) in p.variants.iter().enumerate() {
            let mut row = vec![String::new(); width];
            row[1] = p.handle.clone();
            if i == 0 {
                row[0] = p.title.clone();
                row[2] = p.description.clone();
                row[3] = p.vendor.clone();
                row[4] = p.category.clone();
                row[5] = p.product_type.into();
                row[6] = p.tags.join(", ");
                row[7] = active.to_string();
                row[8] = if active { "active" } else { "draft" }.into();
                row[10] = option(v).0.into();
                if let Some(image) = p.images.first() {
                    row[17] = image.clone();
                    row[18] = "1".into();
                    row[19] = format!("{} — front", p.title);
                }
            }
            row[9] = v.sku.clone();
            row[11] = option(v).1.into();
            row[12] = format!("{:.2}", (v.price * 100.0).round() / 100.0);
            row[13] = "shopify".into();
            row[14] = v.quantity.to_string();
            row[15] = "deny".into();
            row[16] = "true".into();
            row[20] = v.image.clone().unwrap_or_default();
            text.push_str(&csv_line(&row));
        }
        for (i, image) in p.images.iter().enumerate().skip(1) {
            let mut row = vec![String::new(); width];
            row[1] = p.handle.clone();
            row[17] = image.clone();
            row[18] = (i + 1).to_string();
            row[19] = p.title.clone();
            text.push_str(&csv_line(&row));
        }
    }
    text
}

/// The inventory CSV: each variant's stock at `location` becomes the number of copies.
pub fn inventory_csv(products: &[Product], location: &str) -> String {
    let mut text = csv_line(
        &INVENTORY_HEADER
            .iter()
            .map(|h| h.to_string())
            .collect::<Vec<_>>(),
    );
    for p in products {
        for v in &p.variants {
            let (name, value) = option(v);
            let mut row = vec![String::new(); INVENTORY_HEADER.len()];
            row[0] = p.handle.clone();
            row[1] = p.title.clone();
            row[2] = name.into();
            row[3] = value.into();
            row[8] = v.sku.clone();
            row[11] = location.to_string();
            row[18] = v.quantity.to_string();
            text.push_str(&csv_line(&row));
        }
    }
    text
}

pub fn preview(item: &ItemSummary) -> Preview {
    let plan = listing::plan(item);
    let (category, condition) = match &plan {
        Ok(p) => {
            let leaf = self::category(item, p.kind)
                .rsplit(" > ")
                .next()
                .unwrap_or_default()
                .to_string();
            let condition = match (graded_condition(item), &p.graded) {
                (Some(g), None) => format!("{} (from grading)", condition_name(p.kind, g)),
                _ => p.condition_label(),
            };
            (Some(leaf), Some(condition))
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
            .map(|p| (p * 100.0).round() / 100.0),
        ebay: None,
        problem: plan.err().map(|e| e.replace("on eBay ", "on Shopify ")),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Export {
    pub products: usize,
    pub variants: usize,
    pub skipped: Vec<Skipped>,
    /// Products without pictures.
    pub without_images: usize,
    pub image_error: Option<String>,
}

/// The chosen file as CSV text. Products upload the cards' pictures first (once per card); `progress` gets
/// (cards done, cards).
pub async fn export(
    runner: &Runner,
    priced: &[PricedItem],
    options: &Options,
    cancel: &CancellationToken,
    progress: impl Fn(usize, usize),
) -> crate::error::Result<(String, Export)> {
    let products_file = options.file == FileKind::Products;
    if !products_file && options.location.trim().is_empty() {
        return Err(crate::error::Error::msg(
            "enter the store location, exactly as Shopify names it",
        ));
    }
    let mut hosting = Hosting::new(runner, options.upload_images && products_file)?;
    let mut export = Export::default();
    let mut cards = Vec::new();
    for (done, p) in priced.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(ApiError::Cancelled.into());
        }
        let item = runner.db.item(p.item_id)?;
        let skip = |export: &mut Export, reason: String| {
            export.skipped.push(Skipped {
                seq: item.summary.seq,
                reason,
            })
        };
        match listing::plan(&item.summary) {
            Err(why) => skip(&mut export, why.replace("on eBay ", "on Shopify ")),
            Ok(_) if products_file && !(p.price.is_finite() && p.price > 0.0) => {
                skip(&mut export, "no price".into())
            }
            Ok(plan) => {
                let images = if products_file {
                    hosting.urls(runner, &item, cancel).await?
                } else {
                    Vec::new()
                };
                let condition = graded_condition(&item.summary).unwrap_or(options.condition);
                cards.push(Card {
                    item,
                    plan,
                    price: p.price,
                    condition,
                    images,
                });
            }
        }
        progress(done + 1, priced.len());
    }
    let products = products(&cards, options.grouping);
    export.products = products.len();
    export.variants = products.iter().map(|p| p.variants.len()).sum();
    export.without_images = products.iter().filter(|p| p.images.is_empty()).count();
    export.image_error = hosting.error;
    let text = if products_file {
        product_csv(&products, options.active)
    } else {
        export.without_images = 0;
        inventory_csv(&products, options.location.trim())
    };
    Ok((text, export))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::test_item as item;
    use crate::model::SlabLabel;

    fn card(event: &str, condition: CardCondition, price: f64, slab: Option<&str>) -> Card {
        let mut item = item("tcg", "Pokemon");
        item.event_id = event.into();
        item.summary.slab = slab.map(|cert| SlabLabel {
            company: "PSA".into(),
            grade: "10".into(),
            certificate: cert.into(),
        });
        let plan = listing::plan(&item.summary).unwrap();
        let images = vec![
            format!("https://i.ibb.co/{event}/front.jpg"),
            format!("https://i.ibb.co/{event}/back.jpg"),
        ];
        Card {
            item,
            plan,
            price,
            condition,
            images,
        }
    }

    fn cards() -> Vec<Card> {
        vec![
            card("a", CardCondition::NearMint, 10.0, None),
            card("b", CardCondition::Excellent, 6.0, None),
            card("c", CardCondition::NearMint, 12.0, None),
            card("d", CardCondition::NearMint, 300.0, Some("87654321")),
        ]
    }

    #[test]
    fn copies_of_a_card_become_condition_variants_and_slabs_their_own_product() {
        let products = products(&cards(), Grouping::ByCard);
        assert_eq!(products.len(), 2);
        let card = &products[0];
        assert_eq!(card.handle, "pokemon-guilds-of-ravnica-leapfrog-42");
        assert_eq!(
            card.category,
            "Arts & Entertainment > Hobbies & Creative Arts > Collectibles > Collectible Trading Cards > Gaming Cards"
        );
        let summary: Vec<_> = card
            .variants
            .iter()
            .map(|v| (v.condition.unwrap(), v.quantity, v.price, v.sku.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                (
                    "Near Mint",
                    2,
                    12.0,
                    "pokemon-guilds-of-ravnica-leapfrog-42-nm"
                ),
                (
                    "Lightly Played",
                    1,
                    6.0,
                    "pokemon-guilds-of-ravnica-leapfrog-42-lp"
                ),
            ]
        );
        assert_eq!(card.images.len(), 6, "every copy's pictures");
        let slab = &products[1];
        assert_eq!(
            slab.handle,
            "pokemon-guilds-of-ravnica-leapfrog-42-psa-10-87654321"
        );
        assert_eq!(slab.variants[0].condition, None);
        assert_eq!(slab.product_type, "Graded Card");
        assert!(slab.tags.contains(&"PSA 10".to_string()));
    }

    #[test]
    fn per_card_products_have_one_variant_each() {
        let products = products(&cards(), Grouping::PerCard);
        assert_eq!(products.len(), 4);
        assert_eq!(
            products[0].handle,
            "pokemon-guilds-of-ravnica-leapfrog-42-a"
        );
        assert!(products
            .iter()
            .all(|p| p.variants.len() == 1 && p.variants[0].quantity == 1));
        assert_eq!(products[0].variants[0].sku, "cl-s-a");
    }

    #[test]
    fn the_product_csv_has_variant_and_picture_rows() {
        let text = product_csv(&products(&cards()[..2], Grouping::ByCard), false);
        let mut reader = csv::Reader::from_reader(text.as_bytes());
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            PRODUCT_HEADER
        );
        let rows: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
        assert_eq!(rows.len(), 2 + 3, "two variants, then three more pictures");
        assert_eq!(&rows[0][0], "2018 Pokemon Guilds of Ravnica Leapfrog #42");
        assert_eq!((&rows[0][8], &rows[0][7]), ("draft", "false"));
        assert_eq!(
            (&rows[0][10], &rows[0][11], &rows[0][12], &rows[0][14]),
            ("Condition", "Near Mint", "10.00", "1")
        );
        assert_eq!(
            (&rows[0][17], &rows[0][18]),
            ("https://i.ibb.co/a/front.jpg", "1")
        );
        assert_eq!(
            (&rows[1][0], &rows[1][1], &rows[1][11]),
            (
                "",
                "pokemon-guilds-of-ravnica-leapfrog-42",
                "Lightly Played"
            )
        );
        assert_eq!(&rows[1][20], "https://i.ibb.co/b/front.jpg");
        assert_eq!(
            (&rows[2][11], &rows[2][17], &rows[2][18]),
            ("", "https://i.ibb.co/a/back.jpg", "2")
        );
    }

    #[test]
    fn the_inventory_csv_sets_each_variants_stock() {
        let text = inventory_csv(&products(&cards(), Grouping::ByCard), "Shop location");
        let mut reader = csv::Reader::from_reader(text.as_bytes());
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            INVENTORY_HEADER
        );
        let rows: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            (&rows[0][0], &rows[0][2], &rows[0][3]),
            (
                "pokemon-guilds-of-ravnica-leapfrog-42",
                "Condition",
                "Near Mint"
            )
        );
        assert_eq!(
            (&rows[0][11], &rows[0][17], &rows[0][18]),
            ("Shop location", "", "2")
        );
        assert_eq!(
            (&rows[2][2], &rows[2][3], &rows[2][18]),
            ("Title", "Default Title", "1")
        );
    }

    #[test]
    fn handles_are_plain() {
        assert_eq!(
            handle(&["Pokémon", "Base Set", "Pikachu", "58/102"]),
            "pokemon-base-set-pikachu-58-102"
        );
        assert_eq!(handle(&["YuGiOh!", "", "Blue-Eyes"]), "yugioh-blue-eyes");
    }
}
