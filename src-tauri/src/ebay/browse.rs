//! Active eBay listings of a card, and the range of their prices: the Browse API's search
//! (`buy/browse/v1/item_summary/search`) with an application token, so no seller account is needed.
//!
//! The token comes from the user's keyset (App ID and Cert ID in Settings → eBay), or from `EBAY_APP_ID` in the
//! environment: the base64 of `client_id:client_secret`, as Ximilar's pricing crawlers read it. The query is built
//! from the identified card; an AI agent can then keep only the listings that are the card (`keep`).

use std::time::{Duration, Instant};

use serde_json::Value;

use super::client::{self, EbayClient};
use super::Hosts;
use crate::model::{EbayMarket, ItemSummary, MarketListing, PriceRange, SlabLabel};
use crate::settings::{EbaySettings, Secret, SettingsStore};
use crate::ximilar::http::{detail, ApiError};

const APP_SCOPE: &str = "https://api.ebay.com/oauth/api_scope";
/// The environment variable with an application key: base64 of `client_id:client_secret`.
pub const APP_KEY_ENV: &str = "EBAY_APP_ID";
/// eBay's condition ids for trading and sports card singles: in a slab, or not.
const GRADED: &str = "2750";
const UNGRADED: &str = "4000";
/// Listings kept per search: enough for a range, few enough for an agent to read.
pub const DEFAULT_LIMIT: u32 = 50;
pub const MAX_LIMIT: u32 = 200;

/// How the application authenticates: the Basic credentials and the hosts they belong to.
struct AppKey {
    basic: String,
    cfg: EbaySettings,
}

/// `EBAY_APP_ID` (production eBay), else the keyset in Settings; none without either. Tests leave the
/// developer's own key alone unless they are the real-eBay one (`CARDLOADER_REAL_EBAY=1`), and a mock host
/// (`api_base`) never gets it.
fn app_key(settings: &SettingsStore) -> Option<AppKey> {
    use base64::Engine;
    let cfg = settings.get().ebay;
    let env_allowed = !cfg!(test) || std::env::var("CARDLOADER_REAL_EBAY").is_ok_and(|v| v == "1");
    if env_allowed && cfg.api_base.is_empty() {
        if let Ok(key) = std::env::var(APP_KEY_ENV) {
            let key = key.trim().to_string();
            if !key.is_empty() {
                return Some(AppKey {
                    basic: key,
                    cfg: EbaySettings {
                        sandbox: false,
                        ..cfg
                    },
                });
            }
        }
    }
    let cert = settings.secret(Secret::EbayCert)?;
    if cfg.app_id.is_empty() {
        return None;
    }
    let basic = base64::engine::general_purpose::STANDARD.encode(format!("{}:{cert}", cfg.app_id));
    Some(AppKey { basic, cfg })
}

/// Whether listings can be searched at all.
pub fn available(settings: &SettingsStore) -> bool {
    app_key(settings).is_some()
}

pub const NO_KEY: &str =
    "enter your eBay App ID and Cert ID in Settings → eBay to search eBay listings";

/// The application token (2 h), kept until shortly before it expires.
#[derive(Default)]
pub struct AppToken {
    current: tokio::sync::Mutex<Option<(String, String, Instant)>>,
}

impl AppToken {
    pub async fn client(&self, settings: &SettingsStore) -> Result<EbayClient, ApiError> {
        let key = app_key(settings).ok_or_else(|| ApiError::Status {
            status: 401,
            detail: NO_KEY.into(),
        })?;
        let mut current = self.current.lock().await;
        if let Some((basic, token, until)) = current.as_ref() {
            if *basic == key.basic && *until > Instant::now() + Duration::from_secs(300) {
                return EbayClient::new(&key.cfg, token);
            }
        }
        let resp = client::http()?
            .post(format!(
                "{}/identity/v1/oauth2/token",
                Hosts::of(&key.cfg).api
            ))
            .header("Authorization", format!("Basic {}", key.basic))
            .form(&[("grant_type", "client_credentials"), ("scope", APP_SCOPE)])
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        if !status.is_success() {
            return Err(ApiError::Status {
                status: status.as_u16(),
                detail: format!("eBay refused the keyset: {}", detail(status, &text)),
            });
        }
        let body: Value =
            serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))?;
        let token = body["access_token"]
            .as_str()
            .ok_or_else(|| ApiError::Decode("eBay sent no access token".into()))?
            .to_string();
        let until =
            Instant::now() + Duration::from_secs(body["expires_in"].as_u64().unwrap_or(7200));
        *current = Some((key.basic.clone(), token.clone(), until));
        EbayClient::new(&key.cfg, &token)
    }
}

/// What to search for.
#[derive(Debug, Clone, PartialEq)]
pub struct Search {
    pub query: String,
    pub category_id: Option<&'static str>,
    /// `GRADED` or `UNGRADED` for card singles; none for comics.
    pub condition_id: Option<&'static str>,
    /// Auctions too: their current bid is rarely what the card sells for, so they are left out by default.
    pub auctions: bool,
    pub limit: u32,
}

/// The search for an identified card: its name, set and number (and a sports card's year and parallel), in its
/// eBay category; a card in a slab among graded listings with its grader and grade, any other among ungraded ones.
pub fn search_for(item: &ItemSummary) -> Option<Search> {
    let c = &item.card;
    if c.name.trim().is_empty() && c.full_name.trim().is_empty() {
        return None;
    }
    let kind = item.id_type.as_deref().unwrap_or("tcg");
    let words: Vec<&str> = match kind {
        "sport" => vec![&c.year, &c.set_name, &c.name, &c.series, &c.card_number],
        "comics" => vec![&c.name, &c.card_number, &c.set_name],
        _ => vec![
            &c.name,
            if c.set_code.is_empty() {
                &c.set_name
            } else {
                &c.set_code
            },
            &c.card_number,
        ],
    }
    .into_iter()
    .map(String::as_str)
    .collect();
    let mut query = join(&words);
    if query.is_empty() {
        query = c.full_name.trim().to_string();
    }
    if let Some(SlabLabel { company, grade, .. }) = &item.slab {
        query = join(&[&query, company, grade]);
    }
    let comics = kind == "comics";
    Some(Search {
        query,
        category_id: Some(match kind {
            "sport" => "261328",
            "comics" => "63",
            _ => "183454",
        }),
        condition_id: (!comics).then_some(if item.slab.is_some() {
            GRADED
        } else {
            UNGRADED
        }),
        auctions: false,
        limit: DEFAULT_LIMIT,
    })
}

fn join(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Searches eBay; returns how many listings it has, and the first `limit` of them.
pub async fn search(
    client: &EbayClient,
    s: &Search,
) -> Result<(i64, Vec<MarketListing>), ApiError> {
    let mut filters = vec![if s.auctions {
        "buyingOptions:{FIXED_PRICE|AUCTION}".to_string()
    } else {
        "buyingOptions:{FIXED_PRICE}".to_string()
    }];
    if let Some(id) = s.condition_id {
        filters.push(format!("conditionIds:{{{id}}}"));
    }
    let mut query = vec![
        ("q", s.query.clone()),
        ("limit", s.limit.clamp(1, MAX_LIMIT).to_string()),
        ("filter", filters.join(",")),
    ];
    if let Some(category) = s.category_id {
        query.push(("category_ids", category.to_string()));
    }
    let body = client
        .request(
            reqwest::Method::GET,
            "buy/browse/v1/item_summary/search",
            &query,
            None,
        )
        .await?;
    let listings: Vec<MarketListing> = body["itemSummaries"]
        .as_array()
        .map(|items| items.iter().filter_map(listing).collect())
        .unwrap_or_default();
    // eBay's total is an estimate, at times below what it just returned.
    let total = body["total"]
        .as_i64()
        .unwrap_or_default()
        .max(listings.len() as i64);
    Ok((total, listings))
}

/// One item summary; none without a price.
fn listing(v: &Value) -> Option<MarketListing> {
    let price = if v["price"].is_object() {
        &v["price"]
    } else {
        &v["currentBidPrice"]
    };
    let value = price["value"]
        .as_str()
        .and_then(|p| p.parse::<f64>().ok())
        .or_else(|| price["value"].as_f64())?;
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let image = v["image"]["imageUrl"]
        .as_str()
        .or(v["thumbnailImages"][0]["imageUrl"].as_str())
        .map(str::to_string);
    Some(MarketListing {
        id: s(&v["itemId"]),
        title: s(&v["title"]),
        price: value,
        currency: s(&price["currency"]),
        url: s(&v["itemWebUrl"]),
        image,
        condition: s(&v["condition"]),
        buying: v["buyingOptions"]
            .as_array()
            .map(|o| {
                o.iter()
                    .filter_map(|b| b.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        seller: s(&v["seller"]["username"]),
        country: s(&v["itemLocation"]["country"]),
        counted: true,
    })
}

/// A search's result as the card keeps it.
pub fn market(s: &Search, total: i64, listings: Vec<MarketListing>) -> EbayMarket {
    EbayMarket {
        query: s.query.clone(),
        category_id: s.category_id.map(str::to_string),
        searched_at: crate::model::now(),
        total,
        range: PriceRange::of(&listings),
        listings,
        note: String::new(),
    }
}

/// Only the listings with these ids count (the others are other cards, lots, …); `note` says why.
pub fn keep(market: &mut EbayMarket, ids: &[String], note: &str) -> Result<(), String> {
    let unknown: Vec<&String> = ids
        .iter()
        .filter(|id| !market.listings.iter().any(|l| &l.id == *id))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "no such listing in the card's last search: {}",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for l in &mut market.listings {
        l.counted = ids.contains(&l.id);
    }
    market.note = note.trim().to_string();
    market.range = PriceRange::of(&market.listings);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CardFields, ItemStatus};
    use serde_json::json;

    fn item(id_type: &str, card: CardFields, slab: Option<SlabLabel>) -> ItemSummary {
        let mut item: ItemSummary =
            serde_json::from_value(json!({"id": 1, "sessionId": "s", "seq": 1, "status": "done",
                "frontPath": "a.jpg", "backPath": null, "thumbPath": null, "idType": id_type,
                "name": "", "fullName": "", "setName": "", "setCode": "", "cardNumber": "", "series": "",
                "year": "", "subcategory": "", "confidence": null, "distance": null, "price": null,
                "grade": null, "condition": null, "slab": null, "error": null, "synced": false,
                "updatedAt": "", "picked": false, "edited": false, "reviewed": false, "agentReviewed": false,
                "reviewNote": "", "notes": "", "location": "", "ebay": null, "marketRange": null}))
            .unwrap();
        item.status = ItemStatus::Done;
        item.card = card;
        item.slab = slab;
        item
    }

    #[test]
    fn searches_by_the_card() {
        let tcg = item(
            "tcg",
            CardFields {
                name: "Charizard ex".into(),
                set_name: "Obsidian Flames".into(),
                set_code: "OBF".into(),
                card_number: "125".into(),
                ..CardFields::default()
            },
            None,
        );
        let s = search_for(&tcg).unwrap();
        assert_eq!(s.query, "Charizard ex OBF 125");
        assert_eq!(
            (s.category_id, s.condition_id),
            (Some("183454"), Some(UNGRADED))
        );
        let sport = item(
            "sport",
            CardFields {
                name: "Patrick Mahomes II".into(),
                set_name: "Panini Prizm".into(),
                card_number: "269".into(),
                series: "Silver Prizm".into(),
                year: "2017".into(),
                ..CardFields::default()
            },
            Some(SlabLabel {
                company: "PSA".into(),
                grade: "10".into(),
                certificate: "1".into(),
            }),
        );
        let s = search_for(&sport).unwrap();
        assert_eq!(
            s.query,
            "2017 Panini Prizm Patrick Mahomes II Silver Prizm 269 PSA 10"
        );
        assert_eq!(
            (s.category_id, s.condition_id),
            (Some("261328"), Some(GRADED))
        );
        let comic = item(
            "comics",
            CardFields {
                name: "Amazing Spider-Man".into(),
                card_number: "300".into(),
                ..CardFields::default()
            },
            None,
        );
        assert_eq!(search_for(&comic).unwrap().condition_id, None);
        assert!(search_for(&item("tcg", CardFields::default(), None)).is_none());
    }

    #[test]
    fn reads_listings_and_keeps_the_card() {
        let body = json!({"total": 3, "itemSummaries": [
            {"itemId": "v1|1|0", "title": "Charizard ex 125/197", "price": {"value": "20.00", "currency": "USD"},
             "itemWebUrl": "https://www.ebay.com/itm/1", "buyingOptions": ["FIXED_PRICE"],
             "image": {"imageUrl": "https://i.ebayimg.com/1.jpg"}, "condition": "Ungraded"},
            {"itemId": "v1|2|0", "title": "Charizard ex 125/197 NM", "price": {"value": "30.50", "currency": "USD"},
             "itemWebUrl": "https://www.ebay.com/itm/2"},
            {"itemId": "v1|3|0", "title": "Charizard lot x10", "price": {"value": "100.00", "currency": "USD"}},
            {"itemId": "v1|4|0", "title": "no price"}]});
        let listings: Vec<MarketListing> = body["itemSummaries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(listing)
            .collect();
        assert_eq!(listings.len(), 3);
        let s = Search {
            query: "Charizard ex OBF 125".into(),
            category_id: Some("183454"),
            condition_id: Some(UNGRADED),
            auctions: false,
            limit: 50,
        };
        let mut m = market(&s, 3, listings);
        let r = m.range.clone().unwrap();
        assert_eq!((r.min, r.max, r.median, r.count), (20.0, 100.0, 30.5, 3));
        keep(
            &mut m,
            &["v1|1|0".into(), "v1|2|0".into()],
            "the lot is ten cards",
        )
        .unwrap();
        let r = m.range.clone().unwrap();
        assert_eq!(
            (r.min, r.max, r.average, r.median, r.count),
            (20.0, 30.5, 25.25, 25.25, 2)
        );
        assert!(keep(&mut m, &["v1|9|0".into()], "").is_err());
    }

    /// Searches real eBay with `EBAY_APP_ID` (or `--ignored` fails): `CARDLOADER_REAL_EBAY=1 EBAY_APP_ID=…
    /// cargo test -p cardloader real_ebay_search -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn real_ebay_search() {
        let dir = tempfile::tempdir().unwrap();
        let settings = SettingsStore::for_test(
            dir.path().join("settings.json"),
            crate::settings::Settings::default(),
            "token",
        );
        let client = AppToken::default().client(&settings).await.unwrap();
        for (kind, card) in [
            (
                "tcg",
                CardFields {
                    name: "Charizard ex".into(),
                    set_code: "OBF".into(),
                    card_number: "125".into(),
                    ..CardFields::default()
                },
            ),
            (
                "sport",
                CardFields {
                    name: "Patrick Mahomes II".into(),
                    set_name: "Panini Prizm".into(),
                    year: "2017".into(),
                    card_number: "269".into(),
                    ..CardFields::default()
                },
            ),
        ] {
            let s = search_for(&item(kind, card, None)).unwrap();
            let (total, listings) = search(&client, &s).await.unwrap();
            let m = market(&s, total, listings);
            println!(
                "{} → {} on eBay, {} kept, range {:?}",
                s.query,
                m.total,
                m.listings.len(),
                m.range
            );
            for l in m.listings.iter().take(5) {
                println!(
                    "  {:>8.2} {} {} | {}",
                    l.price, l.currency, l.condition, l.title
                );
            }
            assert!(m.range.is_some(), "{} found nothing", s.query);
        }
    }
}
