//! Active eBay listings of a card, and the range of their prices: the Browse API's search
//! (`buy/browse/v1/item_summary/search`) with an application token, so no seller account is needed.
//!
//! The token comes from the user's keyset (App ID and Cert ID in Settings → Selling), or from `EBAY_APP_ID` in the
//! environment: the base64 of `client_id:client_secret`, as Ximilar's pricing crawlers read it. The query is built
//! from the identified card; an AI agent can then keep only the listings that are the card (`keep`).

use std::time::{Duration, Instant};

use serde_json::Value;

use super::client::{self, EbayClient};
use super::listing::Kind;
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
/// An eBay site whose listings can be searched: its marketplace id, its domain, the currency its prices are in, and
/// the language its listings are read in.
#[derive(Debug, PartialEq, Eq)]
pub struct SearchMarket {
    pub id: &'static str,
    pub site: &'static str,
    pub currency: &'static str,
    pub language: &'static str,
}

const fn site(
    id: &'static str,
    site: &'static str,
    currency: &'static str,
    language: &'static str,
) -> SearchMarket {
    SearchMarket {
        id,
        site,
        currency,
        language,
    }
}

/// The sites the Browse API searches (eBay's "Buy API marketplace support"). Only EBAY_US's category and
/// condition ids are known here; on the others the category comes from eBay's own suggestion (`category_on`).
pub const SEARCH_MARKETS: &[SearchMarket] = &[
    site("EBAY_US", "ebay.com", "USD", "en-US"),
    site("EBAY_GB", "ebay.co.uk", "GBP", "en-GB"),
    site("EBAY_DE", "ebay.de", "EUR", "de-DE"),
    site("EBAY_FR", "ebay.fr", "EUR", "fr-FR"),
    site("EBAY_IT", "ebay.it", "EUR", "it-IT"),
    site("EBAY_ES", "ebay.es", "EUR", "es-ES"),
    site("EBAY_NL", "ebay.nl", "EUR", "nl-NL"),
    site("EBAY_BE", "ebay.be", "EUR", "nl-BE"),
    site("EBAY_AT", "ebay.at", "EUR", "de-AT"),
    site("EBAY_IE", "ebay.ie", "EUR", "en-IE"),
    site("EBAY_CH", "ebay.ch", "CHF", "de-CH"),
    site("EBAY_PL", "ebay.pl", "PLN", "pl-PL"),
    site("EBAY_CA", "ebay.ca", "CAD", "en-CA"),
    site("EBAY_AU", "ebay.com.au", "AUD", "en-AU"),
    site("EBAY_HK", "ebay.com.hk", "HKD", "zh-HK"),
    site("EBAY_SG", "ebay.com.sg", "SGD", "en-SG"),
];

/// The search site `id` names, else ebay.com.
pub fn search_market(id: &str) -> &'static SearchMarket {
    SEARCH_MARKETS
        .iter()
        .find(|m| m.id.eq_ignore_ascii_case(id.trim()))
        .unwrap_or(&SEARCH_MARKETS[0])
}

impl SearchMarket {
    fn is_us(&self) -> bool {
        self.id == "EBAY_US"
    }
}

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

/// The application token (2 h), kept until shortly before it expires.
#[derive(Default)]
pub struct AppToken {
    current: tokio::sync::Mutex<Option<(String, String, Instant)>>,
}

impl AppToken {
    pub async fn client(&self, settings: &SettingsStore) -> Result<EbayClient, ApiError> {
        let key = app_key(settings).ok_or(ApiError::NoEbayKeys)?;
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
    /// The site searched.
    pub market: &'static SearchMarket,
    /// The card's category there: EBAY_US's known ids; on other sites eBay's suggestion, asked when searching.
    pub category_id: Option<String>,
    /// `GRADED` or `UNGRADED` for card singles; none for comics.
    pub condition_id: Option<&'static str>,
    /// Auctions too: their current bid is rarely what the card sells for, so they are left out by default.
    pub auctions: bool,
    pub limit: u32,
}

/// The search for an identified card on `market`: its name, set and number (and a sports card's year and parallel),
/// in its eBay category; a card in a slab among graded listings with its grader and grade, any other among
/// ungraded ones.
pub fn search_for(item: &ItemSummary, market: &'static SearchMarket) -> Option<Search> {
    let c = &item.card;
    if c.name.trim().is_empty() && c.full_name.trim().is_empty() {
        return None;
    }
    // A card of no known type (a slab whose label names no game) is searched as a trading card.
    let what = Kind::of(item).unwrap_or(Kind::Tcg);
    let words: Vec<&str> = match what {
        Kind::Sport => {
            vec![&c.year, &c.set_name, &c.name, &c.series, &c.card_number]
        }
        Kind::Comic => vec![&c.name, &c.card_number, &c.set_name],
        Kind::Tcg => vec![
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
    Some(Search {
        query,
        market,
        category_id: market.is_us().then(|| what.search_category().to_string()),
        // Comics have no graded/ungraded condition of the cards' kind.
        condition_id: (what != Kind::Comic).then_some(if item.slab.is_some() {
            GRADED
        } else {
            UNGRADED
        }),
        auctions: false,
        limit: DEFAULT_LIMIT,
    })
}

/// The listings are what Cardloader's own search for the card, as it is, finds: a typed correction of its fields
/// makes them another card's. Listings found by someone's own query were chosen for the card they looked at, and
/// stay with it.
pub fn searched_from(item: &ItemSummary, market: &EbayMarket) -> bool {
    search_for(item, search_market(&market.marketplace)).is_some_and(|s| s.query == market.query)
}

fn join(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Searches eBay on the search's site; returns how many listings it has, and the first `limit` of them. Off ebay.com
/// the category is eBay's suggestion for the query, and a search the condition leaves empty is asked again
/// without it (another site may grade cards with other condition ids): `s` says what was searched in the end.
pub async fn search(
    client: &EbayClient,
    s: &mut Search,
) -> Result<(i64, Vec<MarketListing>), ApiError> {
    let client = client.on(s.market.id, s.market.language);
    if s.category_id.is_none() && !s.market.is_us() {
        s.category_id = category_on(&client, s.market, &s.query).await;
    }
    let found = search_once(&client, s).await?;
    if found.0 == 0 && !s.market.is_us() && s.condition_id.is_some() {
        s.condition_id = None;
        return search_once(&client, s).await;
    }
    Ok(found)
}

/// Category trees by site, asked once (`get_default_category_tree_id`).
static TREES: std::sync::Mutex<Vec<(&'static str, String)>> = std::sync::Mutex::new(Vec::new());

/// The category eBay suggests on `market` for `query` (its Taxonomy API); none when it can't say, and the search
/// then runs in every category.
async fn category_on(
    client: &EbayClient,
    market: &'static SearchMarket,
    query: &str,
) -> Option<String> {
    let known = TREES
        .lock()
        .unwrap()
        .iter()
        .find(|(id, _)| *id == market.id)
        .map(|(_, tree)| tree.clone());
    let tree = match known {
        Some(tree) => tree,
        None => {
            let body = client
                .request(
                    reqwest::Method::GET,
                    "commerce/taxonomy/v1/get_default_category_tree_id",
                    &[("marketplace_id", market.id.to_string())],
                    None,
                )
                .await
                .map_err(|e| tracing::warn!("category tree of {}: {e}", market.id))
                .ok()?;
            let tree = body["categoryTreeId"].as_str()?.to_string();
            TREES.lock().unwrap().push((market.id, tree.clone()));
            tree
        }
    };
    let body = client
        .request(
            reqwest::Method::GET,
            &format!("commerce/taxonomy/v1/category_tree/{tree}/get_category_suggestions"),
            &[("q", query.to_string())],
            None,
        )
        .await
        .map_err(|e| tracing::warn!("category of \"{query}\" on {}: {e}", market.id))
        .ok()?;
    body["categorySuggestions"][0]["category"]["categoryId"]
        .as_str()
        .map(str::to_string)
}

async fn search_once(
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
    if let Some(category) = &s.category_id {
        query.push(("category_ids", category.clone()));
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
        marketplace: s.market.id.to_string(),
        category_id: s.category_id.clone(),
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
        let s = search_for(&tcg, search_market("EBAY_US")).unwrap();
        assert_eq!(s.query, "Charizard ex OBF 125");
        assert_eq!(
            (s.category_id.as_deref(), s.condition_id),
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
        let s = search_for(&sport, search_market("EBAY_US")).unwrap();
        assert_eq!(
            s.query,
            "2017 Panini Prizm Patrick Mahomes II Silver Prizm 269 PSA 10"
        );
        let found = |query: &str| EbayMarket {
            query: query.into(),
            marketplace: "EBAY_US".into(),
            ..EbayMarket::default()
        };
        assert!(searched_from(&sport, &found(&s.query)));
        assert!(!searched_from(
            &sport,
            &found("Mahomes Prizm 269 silver PSA 10")
        ));
        assert_eq!(
            (s.category_id.as_deref(), s.condition_id),
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
        let s = search_for(&comic, search_market("EBAY_US")).unwrap();
        assert_eq!(
            (s.category_id.as_deref(), s.condition_id),
            (Some("63"), None)
        );
        // Off ebay.com the category is eBay's own suggestion, asked when searching.
        let de = search_for(&comic, search_market("ebay_de")).unwrap();
        assert_eq!((de.market.currency, de.category_id), ("EUR", None));
        assert_eq!(search_market("nowhere").id, "EBAY_US");
        // A slab is typed by its sport or game, as for listing it: a graded basketball card is a sports card.
        let slab = item(
            "slab",
            CardFields {
                name: "LeBron James".into(),
                set_name: "Topps Chrome".into(),
                card_number: "111".into(),
                year: "2003".into(),
                subcategory: "Basketball".into(),
                ..CardFields::default()
            },
            Some(SlabLabel {
                company: "PSA".into(),
                grade: "10".into(),
                certificate: "1".into(),
            }),
        );
        let s = search_for(&slab, search_market("EBAY_US")).unwrap();
        assert_eq!(s.query, "2003 Topps Chrome LeBron James 111 PSA 10");
        assert_eq!(
            (s.category_id.as_deref(), s.condition_id),
            (Some("261328"), Some(GRADED))
        );
        assert!(search_for(
            &item("tcg", CardFields::default(), None),
            search_market("EBAY_US")
        )
        .is_none());
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
            market: search_market("EBAY_US"),
            category_id: Some("183454".into()),
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

    /// Searches real eBay with `EBAY_APP_ID` (or `--ignored` fails), on ebay.com or `CARDLOADER_EBAY_SITE` (e.g.
    /// EBAY_DE): `CARDLOADER_REAL_EBAY=1 EBAY_APP_ID=… cargo test -p cardloader real_ebay_search -- --ignored
    /// --nocapture`.
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
            let site = search_market(&std::env::var("CARDLOADER_EBAY_SITE").unwrap_or_default());
            let mut s = search_for(&item(kind, card, None), site).unwrap();
            let (total, listings) = search(&client, &mut s).await.unwrap();
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
    /// What eBay says of the category comics are listed in (`listing::Kind::Comic`), to check the conditions and
    /// item specifics `listing.rs` sends, which come from eBay's pages and listings, not its documentation:
    /// `CARDLOADER_REAL_EBAY=1 EBAY_APP_ID=… cargo test -p cardloader real_ebay_comics_category -- --ignored
    /// --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn real_ebay_comics_category() {
        let dir = tempfile::tempdir().unwrap();
        let settings = SettingsStore::for_test(
            dir.path().join("settings.json"),
            crate::settings::Settings::default(),
            "token",
        );
        let client = AppToken::default().client(&settings).await.unwrap();
        let category = crate::ebay::listing::Kind::Comic.category();
        let get = |path: String, query: Vec<(&'static str, String)>| {
            let client = &client;
            async move {
                client
                    .request(reqwest::Method::GET, &path, &query, None)
                    .await
                    .unwrap()
            }
        };
        let tree = get(
            "commerce/taxonomy/v1/category_tree/0/get_category_subtree".into(),
            vec![("category_id", category.into())],
        )
        .await;
        let node = &tree["categorySubtreeNode"];
        println!(
            "{category}: {} (leaf: {})",
            node["category"]["categoryName"], node["leafCategoryTreeNode"]
        );
        let aspects = get(
            "commerce/taxonomy/v1/category_tree/0/get_item_aspects_for_category".into(),
            vec![("category_id", category.into())],
        )
        .await;
        for a in aspects["aspects"].as_array().into_iter().flatten() {
            let c = &a["aspectConstraint"];
            let values: Vec<&str> = a["aspectValues"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v["localizedValue"].as_str())
                .take(30)
                .collect();
            println!(
                "  {} (required: {}, mode: {}): {}",
                a["localizedAspectName"],
                c["aspectRequired"],
                c["aspectMode"],
                values.join(" | ")
            );
        }
        let conditions = get(
            "sell/metadata/v1/marketplace/EBAY_US/get_item_condition_policies".into(),
            vec![("filter", format!("categoryIds:{{{category}}}"))],
        )
        .await;
        println!("{conditions:#}");
        assert_eq!(
            node["leafCategoryTreeNode"], true,
            "{category} isn't a leaf"
        );
    }
}
