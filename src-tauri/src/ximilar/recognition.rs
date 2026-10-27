//! Collectibles recognition (`/collectibles/v2/*`, synchronous): routing, request building, parsing.

use reqwest::Method;
use serde_json::{json, Map, Value};
use tokio_util::sync::CancellationToken;

use super::http::{ApiError, XimilarClient};
use crate::model::{CardFields, IdMode, SlabLabel};

/// Confidence tiers on the best-match distance (lower is closer).
pub const HIGH_MAX_DISTANCE: f64 = 0.30;
pub const MEDIUM_MAX_DISTANCE: f64 = 0.40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    Tcg,
    Sport,
    Comics,
    Slab,
}

impl Endpoint {
    /// From a stored endpoint path such as `collectibles/v2/sport_id`.
    pub fn from_path(path: &str) -> Self {
        match path {
            p if p.ends_with("sport_id") => Endpoint::Sport,
            p if p.ends_with("comics_id") => Endpoint::Comics,
            p if p.ends_with("slab_id") => Endpoint::Slab,
            _ => Endpoint::Tcg,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Endpoint::Tcg => "collectibles/v2/tcg_id",
            Endpoint::Sport => "collectibles/v2/sport_id",
            Endpoint::Comics => "collectibles/v2/comics_id",
            Endpoint::Slab => "collectibles/v2/slab_id",
        }
    }

    /// Cards Cloud's `id_type`.
    pub fn id_type(self) -> &'static str {
        match self {
            Endpoint::Tcg => "tcg",
            Endpoint::Sport => "sport",
            Endpoint::Comics => "comics",
            Endpoint::Slab => "slab",
        }
    }

    /// From Cards Cloud's `id_type`.
    pub fn from_id_type(id_type: &str) -> Option<Self> {
        match id_type {
            "tcg" => Some(Endpoint::Tcg),
            "sport" => Some(Endpoint::Sport),
            "comics" => Some(Endpoint::Comics),
            "slab" => Some(Endpoint::Slab),
            _ => None,
        }
    }

    /// How a stored recognition (`{"endpoint", "id_type"?, "response"}`) is read: by its `id_type` (a custom
    /// endpoint's path says nothing), else by the endpoint path.
    pub fn of_raw(raw: &Value) -> Self {
        raw["id_type"]
            .as_str()
            .and_then(Endpoint::from_id_type)
            .unwrap_or_else(|| Endpoint::from_path(raw["endpoint"].as_str().unwrap_or_default()))
    }

    /// Whether the back of the card is worth sending: sports card backs carry the player, year and number,
    /// while a TCG back is the same on every card.
    pub fn reads_backs(self) -> bool {
        self == Endpoint::Sport
    }

    /// The `Subcategory` values of this endpoint's cards.
    pub fn subcategories(self) -> &'static [&'static str] {
        match self {
            Endpoint::Tcg => TCG_GAMES,
            Endpoint::Sport => SPORTS,
            Endpoint::Slab | Endpoint::Comics => &[],
        }
    }

    /// The category tags this endpoint implies, asserted on a card object so its classifier is skipped.
    pub fn category_attrs(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Endpoint::Tcg => &[
                ("Top Category", "Card"),
                ("Category", "Card/Trading Card Game"),
            ],
            Endpoint::Sport => &[("Top Category", "Card"), ("Category", "Card/Sport Card")],
            Endpoint::Slab => &[("Top Category", "Slab Label")],
            Endpoint::Comics => &[("Top Category", "Comics")],
        }
    }

    pub fn from_mode(mode: IdMode) -> Option<Self> {
        match mode {
            IdMode::Auto => None,
            IdMode::Tcg => Some(Endpoint::Tcg),
            IdMode::Sport => Some(Endpoint::Sport),
            IdMode::Comics => Some(Endpoint::Comics),
            IdMode::Slab => Some(Endpoint::Slab),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RecognitionOptions {
    pub price_stats: bool,
    /// Read the slab label too (slab_id + slab_grade flags) — always on when `/analyze` saw a slab.
    pub slab: bool,
    /// Let sport_id fall back to Ximilar's generative model when the card isn't in its database.
    pub magic_ai: bool,
    /// Send the back with the front (sport_id and custom endpoints).
    pub both_sides: bool,
    /// A recognition endpoint path to call instead of the standard one, e.g. one built for a customer. It
    /// takes the same request and is read the same way as the endpoint the session's mode picks.
    pub custom_endpoint: Option<String>,
    /// Skip Ximilar's detection: the pixel sizes of the images sent, each record then carrying one card object
    /// over the whole image (as Cardstream sends its pre-cropped cards).
    pub skip_detection: Option<ImageSizes>,
    /// Assert the card type of the endpoint (`Endpoint::category_attrs`).
    pub known_category: bool,
    /// The side is known: the front record is "front", the back record "back".
    pub known_side: bool,
    /// The images are upright.
    pub known_rotation: bool,
    /// The writing system of trading cards (`ALPHABETS`), so tcg_id doesn't guess it; None = detected.
    pub alphabet: Option<String>,
    /// The game (TCG) or sport of every card (`Endpoint::subcategories`); sent only to the endpoint it belongs to.
    pub subcategory: Option<String>,
    /// The endpoint finds the card type itself (Auto): no category is asserted. Set by [`recognize`].
    pub detects_type: bool,
}

/// The `Alphabet` values tcg_id knows. It doesn't validate them (an unknown one quietly matches nothing).
pub const ALPHABETS: [&str; 6] = ["latin", "japanese", "chinese", "korean", "russian", "thai"];

/// `Subcategory` values (the game, or the sport) as Ximilar's collectibles taxonomy spells them.
pub const TCG_GAMES: &[&str] = &[
    "Arkham Horror",
    "Digimon",
    "Dragon Ball Super",
    "Duel Masters",
    "Final Fantasy",
    "Flesh and Blood",
    "Force of Will",
    "Garbage Pail Kids",
    "Grand Archive",
    "Lorcana",
    "Lotr",
    "Magic The Gathering",
    "Marvel Champions",
    "Marvel Universe",
    "MetaZoo",
    "Naruto Mythos",
    "One Piece",
    "Pokemon",
    "Star Wars Unlimited",
    "Ultraman",
    "Union Arena",
    "Vanguard",
    "Weiss Schwarz",
    "YuGiOh!",
];
pub const SPORTS: &[&str] = &[
    "Baseball",
    "Basketball",
    "Football",
    "Hockey",
    "MMA",
    "Soccer",
];

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ImageSizes {
    pub front: (u32, u32),
    pub back: Option<(u32, u32)>,
}

/// An `_objects` entry for a card filling the whole image, so the endpoint uses it instead of detecting one.
fn whole_image_card((width, height): (u32, u32)) -> Value {
    json!({"name": "Card", "prob": 1.0, "bound_box": [0, 0, width, height]})
}

impl RecognitionOptions {
    /// The path the request goes to.
    pub fn path(&self, endpoint: Endpoint) -> String {
        match &self.custom_endpoint {
            Some(p) => p.clone(),
            None => endpoint.path().to_string(),
        }
    }

    fn sends_back(&self, endpoint: Endpoint) -> bool {
        self.both_sides && (endpoint.reads_backs() || self.custom_endpoint.is_some())
    }
}

/// A custom endpoint as typed in the UI → a path under the API base URL, if it is one.
pub fn custom_path(text: &str) -> Option<String> {
    let path = text.trim().trim_matches('/');
    let valid = !path.is_empty()
        && !path.contains("..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/_-.".contains(c));
    valid.then(|| path.to_string())
}

/// What the table and Cards Cloud need from a recognition response.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recognition {
    pub id_type: String,
    pub card: CardFields,
    pub distance: Option<f64>,
    pub confidence: Option<String>,
    pub links: Map<String, Value>,
    pub alternatives: Vec<Value>,
    pub price_stats: Vec<Value>,
    pub price: Option<f64>,
    pub slab: Option<SlabLabel>,
}

/// The request body; the back, when sent, is a second record.
pub fn build_request(
    endpoint: Endpoint,
    image_b64: &str,
    back_b64: Option<&str>,
    opts: &RecognitionOptions,
) -> Value {
    let mut body = json!({});
    // Without it the endpoint tags a turned card (`Rotation`) but identifies it as it lies, i.e. wrongly.
    if !opts.known_rotation {
        body["rotate"] = json!(true);
    }
    if opts.magic_ai && endpoint == Endpoint::Sport {
        body["magic_ai"] = json!(true);
    }
    match endpoint {
        Endpoint::Tcg | Endpoint::Sport => {
            if opts.slab {
                body["slab_id"] = json!(true);
                body["slab_grade"] = json!(true);
            }
            if opts.price_stats {
                body["price_stats"] = json!(true);
            }
        }
        Endpoint::Comics => {
            if opts.slab {
                body["slab_id"] = json!(true);
            }
            if opts.price_stats {
                body["price_stats"] = json!(true);
            }
        }
        Endpoint::Slab => {}
    }
    let alphabet = opts
        .alphabet
        .as_deref()
        .filter(|_| endpoint == Endpoint::Tcg && !opts.detects_type);
    let subcategory = opts
        .subcategory
        .as_deref()
        .filter(|s| !opts.detects_type && endpoint.subcategories().contains(s));
    let record = |id: &str, b64: &str, size: Option<(u32, u32)>| {
        let mut record = json!({"_id": id, "_base64": b64});
        if endpoint == Endpoint::Comics {
            // comics_id takes `lang` per record (a language code), not as a root flag.
            record["lang"] = json!("en");
        }
        // What is known about the card, so the endpoint skips classifying it: on the record (as Ximilar's docs
        // show), or on the card object when detection is skipped (as Cardstream sends it).
        let mut known = Map::new();
        if opts.known_category && !opts.detects_type {
            for (key, value) in endpoint.category_attrs() {
                known.insert((*key).into(), json!(value));
            }
        }
        if opts.known_side {
            known.insert("Side".into(), json!(id));
        }
        if opts.known_rotation {
            known.insert("Rotation".into(), json!("rotation_ok"));
        }
        if let Some(a) = alphabet {
            known.insert("Alphabet".into(), json!(a));
        }
        if let Some(sub) = subcategory {
            known.insert("Subcategory".into(), json!(sub));
        }
        let target = match size {
            Some(size) => {
                record["_objects"] = json!([whole_image_card(size)]);
                &mut record["_objects"][0]
            }
            None => &mut record,
        };
        for (key, value) in known {
            target[key] = value;
        }
        record
    };
    let sizes = opts.skip_detection;
    let mut records = vec![record("front", image_b64, sizes.map(|s| s.front))];
    if let Some(back) = back_b64.filter(|_| opts.sends_back(endpoint)) {
        records.push(record("back", back, sizes.and_then(|s| s.back)));
    }
    body["records"] = json!(records);
    body
}

/// The card type of a response's main object, from its tags (`/analyze` and custom endpoints tag what they
/// detect). Returns the endpoint that reads that type and whether a graded slab was seen.
pub fn route_from_tags(resp: &Value) -> (Endpoint, bool) {
    let record = &resp["records"][0];
    let slab = top_name(&record["Graded Slab"])
        .map(|n| n.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
        || objects(record).iter().any(|o| o["name"] == "Slab Label");

    let mut category: Option<String> = record["Category"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| top_name(&record["Category"]));
    let mut top: Option<String> = None;
    for obj in largest_first(objects(record)) {
        if obj["name"] == "Slab Label" {
            continue;
        }
        if category.is_none() {
            category = top_name(&obj["_tags"]["Category"]).or_else(|| top_name(&obj["Category"]));
        }
        if top.is_none() {
            top =
                top_name(&obj["Top Category"]).or_else(|| obj["name"].as_str().map(str::to_owned));
        }
    }
    let endpoint = match (category.as_deref(), top.as_deref()) {
        (Some(c), _) if c.contains("Sport") => Endpoint::Sport,
        (Some(c), _) if c.contains("Comics") => Endpoint::Comics,
        (_, Some(t)) if t.contains("Comics") => Endpoint::Comics,
        _ => Endpoint::Tcg,
    };
    (endpoint, slab)
}

/// Flattens an id-endpoint response to the best match of the main object.
pub fn parse(endpoint: Endpoint, resp: &Value) -> Recognition {
    let ident = ident_pointer(resp)
        .and_then(|p| resp.pointer(&p))
        .unwrap_or(&Value::Null);
    let best = &ident["best_match"];

    let mut out = Recognition {
        id_type: endpoint.id_type().into(),
        slab: slab_label(resp),
        ..Default::default()
    };
    if !best.is_object() {
        return out;
    }
    let s = |key: &str| text(&best[key]);
    out.card = if endpoint == Endpoint::Comics {
        let number = s("number");
        let name = s("name");
        CardFields {
            full_name: [
                name.clone(),
                if number.is_empty() {
                    String::new()
                } else {
                    format!("#{number}")
                },
                s("title"),
            ]
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
            name,
            set_name: s("publisher"),
            card_number: number,
            year: first_non_empty(&[s("date"), s("year")]),
            series: s("origin_date"),
            subcategory: "Comics".into(),
            ..Default::default()
        }
    } else {
        CardFields {
            name: s("name"),
            full_name: s("full_name"),
            set_name: first_non_empty(&[s("set"), s("set_name")]),
            set_code: s("set_code"),
            card_number: first_non_empty(&[s("card_number"), s("card_no")]),
            series: first_non_empty(&[s("series"), s("sub_set"), s("brand")]),
            year: s("year"),
            subcategory: s("subcategory"),
        }
    };
    if out.card.full_name.is_empty() {
        out.card.full_name = out.card.name.clone();
    }
    out.distance = ident["distances"][0]
        .as_f64()
        .filter(|d| d.is_finite() && *d >= 0.0);
    out.confidence = out.distance.map(|d| tier(d).to_string());
    if let Some(links) = best["links"].as_object() {
        out.links = links
            .iter()
            .filter(|(_, v)| v.is_string())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
    }
    out.alternatives = ident["alternatives"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|v| v.is_object())
                .take(10)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    out.price_stats = best["price_stats"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|v| v.is_object())
                .take(10)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    out.price = representative_price(&out.price_stats);
    out
}

/// Another possible match after the best one, as `cardview::candidates` lists them.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// Index into the response's `alternatives` (what `select_alternative` takes).
    pub index: usize,
    pub name: String,
    pub full_name: String,
    pub set_name: String,
    pub set_code: String,
    pub card_number: String,
    pub year: String,
    pub distance: Option<f64>,
    pub confidence: Option<&'static str>,
    pub price: Option<f64>,
}

/// The card's other candidates; `distances[0]` belongs to the best match, so alternative `i` has `distances[i + 1]`.
pub fn candidates(resp: &Value) -> Vec<Candidate> {
    let ident = ident_pointer(resp)
        .and_then(|p| resp.pointer(&p))
        .unwrap_or(&Value::Null);
    let Some(alternatives) = ident["alternatives"].as_array() else {
        return Vec::new();
    };
    alternatives
        .iter()
        .enumerate()
        .filter(|(_, a)| a.is_object())
        .map(|(index, a)| {
            let s = |keys: &[&str]| {
                first_non_empty(&keys.iter().map(|k| text(&a[*k])).collect::<Vec<_>>())
            };
            let full_name = s(&["full_name"]);
            let distance = a["distance"]
                .as_f64()
                .or(ident["distances"][index + 1].as_f64())
                .filter(|d| d.is_finite());
            Candidate {
                index,
                name: first_non_empty(&[
                    s(&["name", "title"]),
                    full_name.clone(),
                    "Unknown".into(),
                ]),
                full_name,
                set_name: s(&["set", "set_name", "publisher"]),
                set_code: s(&["set_code"]),
                card_number: s(&["card_number", "card_no", "number"]),
                year: s(&["year", "date"]),
                distance,
                confidence: distance.map(tier),
                price: representative_price(
                    a["price_stats"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                ),
            }
        })
        .collect()
}

/// The `Rotation` tag of the front's card when Ximilar found it turned (anything but `rotation_ok`), as
/// `cardview::rotationOf` reads it.
pub fn rotation(resp: &Value) -> Option<String> {
    record_rotation(resp, 0).filter(|name| name != "rotation_ok")
}

/// The back's `Rotation` tag, `rotation_ok` included; none when the back wasn't sent or carries no tag.
pub fn back_rotation(resp: &Value) -> Option<String> {
    record_rotation(resp, 1)
}

fn record_rotation(resp: &Value, index: usize) -> Option<String> {
    let all = objects(&resp["records"][index]);
    let sorted = largest_first(all.clone());
    let card = sorted
        .iter()
        .find(|o| o["name"] != "Slab Label" && o["_identification"].is_object())
        .or_else(|| all.iter().find(|o| o["_identification"].is_object()))
        .or_else(|| sorted.iter().find(|o| o["name"] != "Slab Label"))?;
    top_name(&card["_tags"]["Rotation"])
}

/// The user turned the card's images upright: each side's `Rotation` tag (if any, and not on a slab label)
/// says so from now on, keeping what Ximilar found under `found`. Returns whether a tag changed.
pub fn mark_upright(resp: &mut Value) -> bool {
    let mut changed = false;
    let objects = resp["records"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .filter_map(|r| r["_objects"].as_array_mut())
        .flatten();
    for object in objects {
        if object["name"] == "Slab Label" {
            continue;
        }
        if let Some(tag) = object
            .pointer_mut("/_tags/Rotation/0")
            .and_then(Value::as_object_mut)
        {
            if tag.get("name").and_then(Value::as_str) != Some("rotation_ok") {
                let found = tag.insert("name".into(), json!("rotation_ok"));
                tag.insert("found".into(), found.unwrap_or(Value::Null));
                changed = true;
            }
        }
    }
    changed
}

/// The front's slab label, read as `cardview::matchOf` reads it: the company and grade its tags name, unless
/// the label's identification says them.
pub fn slab_label(resp: &Value) -> Option<SlabLabel> {
    let label = objects(&resp["records"][0])
        .into_iter()
        .find(|o| o["name"] == "Slab Label")?;
    let best = &label["_identification"]["best_match"];
    let tag = |name: &str| top_name(&label["_tags"][name]).unwrap_or_default();
    Some(SlabLabel {
        company: first_non_empty(&[tag("Company"), text(&best["company"])]),
        grade: first_non_empty(&[text(&best["grade"]), tag("Grade")]),
        certificate: text(&best["certificate_number"]),
    })
}

pub fn tier(distance: f64) -> &'static str {
    if distance <= HIGH_MAX_DISTANCE {
        "high"
    } else if distance <= MEDIUM_MAX_DISTANCE {
        "medium"
    } else {
        "low"
    }
}

/// The ungraded median, else the graded one, else overall (the rule Cards Cloud prices by).
pub fn representative_price(price_stats: &[Value]) -> Option<f64> {
    let median = |e: &Value| {
        let v = if e["value"].is_object() {
            &e["value"]
        } else {
            e
        };
        v["median"].as_f64().filter(|m| m.is_finite() && *m >= 0.0)
    };
    ["ungraded", "graded", "overall"].iter().find_map(|kind| {
        price_stats
            .iter()
            .filter(|e| e["stats_type"] == *kind)
            .find_map(median)
    })
}

/// Runs recognition for one card. Auto mode is one `/analyze` call (detect, classify and identify), read as the
/// card type it found; only a sports card that wants magic AI or its back read is then sent to sport_id as well,
/// since `/analyze` takes neither. Returns the parsed result, the raw responses and the number of paid calls.
pub async fn recognize(
    client: &XimilarClient,
    mode: IdMode,
    image_b64: &str,
    back_b64: Option<&str>,
    mut opts: RecognitionOptions,
    cancel: &CancellationToken,
) -> Result<(Recognition, Value, i64), ApiError> {
    let mut raw = Map::new();
    let mut calls = 0;
    let (endpoint, path, body, resp) = match Endpoint::from_mode(mode) {
        Some(endpoint) => {
            let body = build_request(endpoint, image_b64, back_b64, &opts);
            let path = opts.path(endpoint);
            let resp = call(client, &path, &body, cancel).await?;
            calls += 1;
            (endpoint, path, body, resp)
        }
        None => {
            // A custom endpoint takes the sports request (back, magic AI) and is read by the type it reports.
            let auto = RecognitionOptions {
                detects_type: true,
                ..opts.clone()
            };
            let (path, body) = match &opts.custom_endpoint {
                Some(p) => (
                    p.clone(),
                    build_request(Endpoint::Sport, image_b64, back_b64, &auto),
                ),
                None => (
                    ANALYZE.to_string(),
                    build_request(Endpoint::Tcg, image_b64, None, &auto),
                ),
            };
            let resp = call(client, &path, &body, cancel).await?;
            calls += 1;
            let (endpoint, slab) = route_from_tags(&resp);
            let wants_sport_id = endpoint == Endpoint::Sport
                && opts.custom_endpoint.is_none()
                && (opts.magic_ai || (opts.both_sides && back_b64.is_some()));
            if wants_sport_id {
                raw.insert("analyze".into(), strip_images(resp));
                opts.slab |= slab;
                let body = build_request(endpoint, image_b64, back_b64, &opts);
                let path = endpoint.path().to_string();
                let resp = call(client, &path, &body, cancel).await?;
                calls += 1;
                (endpoint, path, body, resp)
            } else {
                (endpoint, path, body, resp)
            }
        }
    };
    if resp["magic_ai_used"] == true {
        // Billed as its own operation.
        calls += 1;
        raw.insert("magic_ai_used".into(), json!(true));
    }
    let parsed = parse(endpoint, &resp);
    raw.insert("endpoint".into(), json!(path));
    raw.insert("id_type".into(), json!(endpoint.id_type()));
    if opts.skip_detection.is_some() {
        raw.insert("skip_detection".into(), json!(true));
    }
    if body["records"].as_array().is_some_and(|r| r.len() > 1) {
        raw.insert("sides".into(), json!(["front", "back"]));
    }
    raw.insert("response".into(), strip_images(resp));
    Ok((parsed, Value::Object(raw), calls))
}

const ANALYZE: &str = "collectibles/v2/analyze";

/// One recognition request. It fails only when no record could be read (a back the endpoint won't take is
/// no failure).
async fn call(
    client: &XimilarClient,
    path: &str,
    body: &Value,
    cancel: &CancellationToken,
) -> Result<Value, ApiError> {
    let resp = client
        .request_retry(Method::POST, path, &[], Some(body), cancel)
        .await?;
    let failed = |r: &Value| r["_status"]["code"].as_u64().filter(|c| *c >= 300);
    let records = resp["records"].as_array().cloned().unwrap_or_default();
    if let Some(status) = records.first().and_then(failed) {
        if records.iter().all(|r| failed(r).is_some()) {
            return Err(ApiError::Status {
                status: status as u16,
                detail: text(&records[0]["_status"]["text"]),
            });
        }
    }
    Ok(resp)
}

/// Responses may echo the request image; don't store megabytes of base64.
fn strip_images(mut resp: Value) -> Value {
    if let Some(records) = resp["records"].as_array_mut() {
        for r in records {
            if let Some(obj) = r.as_object_mut() {
                obj.remove("_base64");
            }
        }
    }
    resp
}

/// Where the identification of the card is, as a JSON pointer into the response. Each record (the front,
/// and the back when it was sent) offers its main object; the one with a best match and the smallest
/// distance wins, the front on a tie.
fn ident_pointer(resp: &Value) -> Option<String> {
    let records = resp["records"].as_array()?;
    let distance = |p: &String| {
        let ident = resp.pointer(p).unwrap_or(&Value::Null);
        if !ident["best_match"].is_object() {
            return f64::INFINITY;
        }
        ident["distances"][0].as_f64().unwrap_or(f64::MAX)
    };
    let mut best: Option<(String, f64)> = None;
    for i in 0..records.len() {
        if let Some(p) = record_ident_pointer(resp, i) {
            let d = distance(&p);
            if best.as_ref().is_none_or(|(_, b)| d < *b) {
                best = Some((p, d));
            }
        }
    }
    best.map(|(p, _)| p)
}

/// One record's main object: the largest identified object that isn't a slab label, else any identified
/// object, else the record's own `_identification`.
fn record_ident_pointer(resp: &Value, index: usize) -> Option<String> {
    let record = &resp["records"][index];
    let identified: Vec<(usize, &Value)> = record["_objects"]
        .as_array()
        .map(|a| {
            a.iter()
                .enumerate()
                .filter(|(_, o)| o["_identification"].is_object())
                .collect()
        })
        .unwrap_or_default();
    let main = identified
        .iter()
        .filter(|(_, o)| o["name"] != "Slab Label")
        .fold(None::<(usize, f64)>, |best, (i, o)| match best {
            Some((_, a)) if a >= area(o) => best,
            _ => Some((*i, area(o))),
        })
        .map(|(i, _)| i)
        .or_else(|| identified.first().map(|(i, _)| *i));
    match main {
        Some(i) => Some(format!("/records/{index}/_objects/{i}/_identification")),
        None if record["_identification"].is_object() => {
            Some(format!("/records/{index}/_identification"))
        }
        None => None,
    }
}

/// Makes alternative `index` (an index into the response's `alternatives`) the best match, as when the user
/// picks it. The previous best match becomes the first alternative; `distances` follow, so the stored response
/// still parses to the picked card with its own distance and prices.
pub fn select_alternative(resp: &mut Value, index: usize) -> Result<(), String> {
    let pointer = ident_pointer(resp).ok_or("this card has no identification")?;
    let ident = resp
        .pointer_mut(&pointer)
        .ok_or("this card has no identification")?;
    let alternatives = ident["alternatives"]
        .as_array_mut()
        .filter(|a| a.get(index).is_some_and(Value::is_object))
        .ok_or("no such candidate")?;
    let picked = alternatives.remove(index);
    let previous = std::mem::replace(&mut ident["best_match"], picked);
    if previous.is_object() {
        ident["alternatives"]
            .as_array_mut()
            .expect("checked above")
            .insert(0, previous);
    }
    if let Some(d) = ident["distances"].as_array_mut() {
        if index + 1 < d.len() {
            let picked = d.remove(index + 1);
            let previous = std::mem::replace(&mut d[0], picked);
            d.insert(1, previous);
        } else if !d.is_empty() {
            // No distance for the candidate: the old one would describe another card.
            d[0] = Value::Null;
        }
    }
    Ok(())
}

fn objects(record: &Value) -> Vec<&Value> {
    record["_objects"]
        .as_array()
        .map(|a| a.iter().filter(|o| o.is_object()).collect())
        .unwrap_or_default()
}

fn area(obj: &Value) -> f64 {
    if let Some(a) = obj["area"].as_f64() {
        return a;
    }
    match obj["bound_box"]
        .as_array()
        .map(|b| b.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
    {
        Some(b) if b.len() == 4 => ((b[2] - b[0]) * (b[3] - b[1])).abs(),
        _ => 0.0,
    }
}

fn largest_first(mut objs: Vec<&Value>) -> Vec<&Value> {
    objs.sort_by(|a, b| {
        area(b)
            .partial_cmp(&area(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    objs
}

/// `[{"name": .., "prob": ..}, ..]` → the first name.
fn top_name(v: &Value) -> Option<String> {
    v.as_array()?.first()?["name"].as_str().map(str::to_owned)
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn first_non_empty(values: &[String]) -> String {
    values
        .iter()
        .find(|v| !v.is_empty())
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rotation_and_candidates_as_the_ui_does() {
        let mut resp = json!({"records": [{"_objects": [
            {"name": "Slab Label", "bound_box": [0, 0, 400, 100], "_tags": {"Rotation": [{"name": "rotated_left"}]}},
            {"name": "Card", "bound_box": [0, 100, 300, 500], "_tags": {"Rotation": [{"name": "rotated_upside_down"}]},
             "_identification": {"best_match": {"name": "A"}, "distances": [0.2, null, 0.45],
                "alternatives": [{"title": "B", "publisher": "Marvel", "number": 7}, "not a card", {"full_name": "C 3"}]}}]}]});
        assert_eq!(rotation(&resp).as_deref(), Some("rotated_upside_down"));
        let found = candidates(&resp);
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].index, found[0].name.as_str()), (0, "B"));
        assert_eq!(
            (found[0].set_name.as_str(), found[0].card_number.as_str()),
            ("Marvel", "7")
        );
        assert_eq!((found[0].distance, found[0].confidence), (None, None));
        // Indexes stay those of `alternatives`, which `select_alternative` takes.
        assert_eq!((found[1].index, found[1].name.as_str()), (2, "C 3"));
        assert_eq!(found[1].confidence, None);
        mark_upright(&mut resp);
        assert_eq!(rotation(&resp), None);
        assert_eq!(back_rotation(&resp), None, "no back was sent");

        // A back Ximilar read keeps its own tag; turning the card marks both sides upright.
        let mut both = json!({"records": [
            {"_objects": [{"name": "Card", "_tags": {"Rotation": [{"name": "rotated_upside_down"}]}}]},
            {"_id": "back", "_objects": [{"name": "Card", "_tags": {"Rotation": [{"name": "rotated_upside_down"}]}}]}]});
        assert_eq!(back_rotation(&both).as_deref(), Some("rotated_upside_down"));
        assert!(mark_upright(&mut both));
        assert_eq!(back_rotation(&both).as_deref(), Some("rotation_ok"));
        assert_eq!(
            both["records"][1]["_objects"][0]["_tags"]["Rotation"][0]["found"],
            "rotated_upside_down"
        );
    }

    fn tcg_response() -> Value {
        json!({"records": [{"_width": 389, "_height": 659, "Category": "Card/Trading Card Game",
            "_objects": [
                {"name": "Slab Label", "bound_box": [31, 22, 375, 130], "_tags": {"Company": [{"name": "PSA", "prob": 0.98}]},
                 "_identification": {"best_match": {"name": "DARK GYARADOS", "grade": "9"}}},
                {"name": "Card", "bound_box": [38, 165, 348, 616], "prob": 0.93,
                 "_identification": {
                    "best_match": {"full_name": "Dark Gyarados Team Rocket (RO) #8", "name": "Dark Gyarados", "set": "Team Rocket",
                        "set_code": "RO", "card_number": "8", "series": "Classic", "year": 2000, "subcategory": "Pokemon",
                        "links": {"tcgplayer.com": "https://www.tcgplayer.com/product/84606", "bad": 1},
                        "price_stats": [
                            {"stats_type": "overall", "value": {"median": 30.0}},
                            {"stats_type": "graded", "value": {"median": 24.99}}]},
                    "alternatives": [{"full_name": "Dark Gyarados Team Rocket (RO) #25"}],
                    "distances": [0.35, 0.38]}}]}],
            "status": {"code": 200}})
    }

    #[test]
    fn parses_the_card_not_the_slab_label() {
        let r = parse(Endpoint::Tcg, &tcg_response());
        assert_eq!(r.card.name, "Dark Gyarados");
        assert_eq!(r.card.set_name, "Team Rocket");
        assert_eq!(r.card.year, "2000");
        assert_eq!(r.confidence.as_deref(), Some("medium"));
        assert_eq!(r.price, Some(24.99));
        assert_eq!(r.links.len(), 1);
        assert_eq!(r.alternatives.len(), 1);
        assert_eq!(r.id_type, "tcg");
        let slab = r.slab.unwrap();
        assert_eq!(slab.company, "PSA");
        assert_eq!(slab.grade, "9");
    }

    #[test]
    fn picking_an_alternative_swaps_it_with_the_best_match() {
        let mut resp = tcg_response();
        select_alternative(&mut resp, 0).unwrap();
        let r = parse(Endpoint::Tcg, &resp);
        assert_eq!(r.card.full_name, "Dark Gyarados Team Rocket (RO) #25");
        assert_eq!(r.distance, Some(0.38));
        assert_eq!(r.price, None);
        assert_eq!(
            r.alternatives[0]["full_name"],
            "Dark Gyarados Team Rocket (RO) #8"
        );
        // and back again
        select_alternative(&mut resp, 0).unwrap();
        let r = parse(Endpoint::Tcg, &resp);
        assert_eq!(r.card.full_name, "Dark Gyarados Team Rocket (RO) #8");
        assert_eq!(r.distance, Some(0.35));
        assert_eq!(r.price, Some(24.99));
        // the slab label is untouched
        assert_eq!(
            resp["records"][0]["_objects"][0]["_identification"]["best_match"]["name"],
            "DARK GYARADOS"
        );
        assert!(select_alternative(&mut resp, 5).is_err());
    }

    #[test]
    fn parses_comics() {
        let resp = json!({"records": [{"_objects": [{"name": "Comics", "_identification": {
            "best_match": {"name": "Fantastic Four", "title": "Galactus!", "date": "August 1976", "number": "173", "publisher": "Marvel"},
            "distances": [0.1]}}]}]});
        let r = parse(Endpoint::Comics, &resp);
        assert_eq!(r.card.full_name, "Fantastic Four #173 Galactus!");
        assert_eq!(r.card.set_name, "Marvel");
        assert_eq!(r.card.year, "August 1976");
        assert_eq!(r.confidence.as_deref(), Some("high"));
    }

    #[test]
    fn no_match_is_empty() {
        let r = parse(Endpoint::Sport, &json!({"records": [{"_objects": []}]}));
        assert_eq!(r.card, CardFields::default());
        assert_eq!(r.id_type, "sport");
        assert!(r.confidence.is_none());
        assert!(r.slab.is_none());
    }

    /// Auto is one /analyze call; a sports card that wants magic AI or its back is then read by sport_id too.
    #[tokio::test]
    async fn auto_analyzes_in_one_call() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        let card = |category: &str, name: &str| {
            json!({"records": [{"_objects": [{"name": "Card", "_tags": {"Category": [{"name": category}]},
                "_identification": {"best_match": {"name": name}, "distances": [0.2]}}]}]})
        };
        let respond = |p: &'static str, body: Value| {
            Mock::given(method("POST"))
                .and(path(p))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
        };
        respond(
            "/collectibles/v2/analyze",
            card("Card/Sport Card", "From analyze"),
        )
        .mount(&server)
        .await;
        respond(
            "/collectibles/v2/sport_id",
            card("Card/Sport Card", "From sport_id"),
        )
        .mount(&server)
        .await;
        let client = XimilarClient::new(&server.uri(), "token", None).unwrap();
        let cancel = CancellationToken::new();

        let (r, raw, calls) = recognize(
            &client,
            IdMode::Auto,
            "F",
            Some("B"),
            RecognitionOptions::default(),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(
            (r.card.name.as_str(), r.id_type.as_str(), calls),
            ("From analyze", "sport", 1)
        );
        assert_eq!(raw["endpoint"], ANALYZE);
        let sent: Value =
            serde_json::from_slice(&server.received_requests().await.unwrap()[0].body).unwrap();
        assert_eq!(
            sent["records"].as_array().unwrap().len(),
            1,
            "/analyze takes the front only"
        );

        let opts = RecognitionOptions {
            both_sides: true,
            ..Default::default()
        };
        let (r, raw, calls) = recognize(&client, IdMode::Auto, "F", Some("B"), opts, &cancel)
            .await
            .unwrap();
        assert_eq!((r.card.name.as_str(), calls), ("From sport_id", 2));
        assert_eq!(raw["endpoint"], "collectibles/v2/sport_id");
        assert_eq!(raw["sides"], json!(["front", "back"]));
        assert!(raw["analyze"].is_object());
    }

    #[test]
    fn skipping_detection_sends_the_whole_image_as_the_card() {
        let opts = RecognitionOptions {
            both_sides: true,
            skip_detection: Some(ImageSizes {
                front: (800, 1120),
                back: Some((790, 1100)),
            }),
            ..Default::default()
        };
        let body = build_request(Endpoint::Sport, "F", Some("B"), &opts);
        let boxes: Vec<&Value> = body["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| &r["_objects"][0]["bound_box"])
            .collect();
        assert_eq!(
            boxes,
            [&json!([0, 0, 800, 1120]), &json!([0, 0, 790, 1100])]
        );
        assert_eq!(body["records"][0]["_objects"][0]["name"], "Card");

        let detect = build_request(
            Endpoint::Sport,
            "F",
            Some("B"),
            &RecognitionOptions::default(),
        );
        assert!(detect["records"][0].get("_objects").is_none());
    }

    #[test]
    fn known_attributes_ride_on_the_card() {
        let opts = RecognitionOptions {
            both_sides: true,
            skip_detection: Some(ImageSizes {
                front: (800, 1120),
                back: Some((800, 1120)),
            }),
            known_category: true,
            known_side: true,
            known_rotation: true,
            alphabet: Some("japanese".into()),
            ..Default::default()
        };
        let sport = build_request(Endpoint::Sport, "F", Some("B"), &opts);
        let front = &sport["records"][0]["_objects"][0];
        assert_eq!(front["Side"], "front");
        assert_eq!(front["Rotation"], "rotation_ok");
        assert!(sport.get("rotate").is_none(), "upright cards aren't turned");
        assert_eq!(front["Category"], "Card/Sport Card");
        assert_eq!(front["Top Category"], "Card");
        assert!(
            front.get("Alphabet").is_none(),
            "the alphabet is for trading cards"
        );
        assert_eq!(sport["records"][1]["_objects"][0]["Side"], "back");

        let tcg = build_request(Endpoint::Tcg, "F", None, &opts);
        assert_eq!(tcg["records"][0]["_objects"][0]["Alphabet"], "japanese");
        assert_eq!(
            tcg["records"][0]["_objects"][0]["Category"],
            "Card/Trading Card Game"
        );

        // Auto: the endpoint tells the type, so none is asserted.
        let auto = RecognitionOptions {
            detects_type: true,
            ..opts.clone()
        };
        let analyze = build_request(Endpoint::Tcg, "F", None, &auto);
        let card = &analyze["records"][0]["_objects"][0];
        assert!(card.get("Category").is_none() && card.get("Alphabet").is_none());
        assert_eq!(card["Side"], "front");

        // Without skipping detection they go on the record, as Ximilar's docs show.
        let plain = RecognitionOptions {
            both_sides: true,
            skip_detection: None,
            ..opts.clone()
        };
        let sport = build_request(Endpoint::Sport, "F", Some("B"), &plain);
        let front = &sport["records"][0];
        assert!(front.get("_objects").is_none());
        assert_eq!(front["Category"], "Card/Sport Card");
        assert_eq!(front["Rotation"], "rotation_ok");
        assert_eq!(front["Side"], "front");
        assert_eq!(sport["records"][1]["Side"], "back");
        let tcg = build_request(Endpoint::Tcg, "F", None, &plain);
        assert_eq!(tcg["records"][0]["Alphabet"], "japanese");

        // The game goes to the endpoint it belongs to only.
        let games = RecognitionOptions {
            subcategory: Some("One Piece".into()),
            ..Default::default()
        };
        let tcg = build_request(Endpoint::Tcg, "F", None, &games);
        assert_eq!(tcg["records"][0]["Subcategory"], "One Piece");
        let sport = build_request(Endpoint::Sport, "F", None, &games);
        assert!(sport["records"][0].get("Subcategory").is_none());

        // Nothing is asserted unless asked for.
        let none = build_request(Endpoint::Tcg, "F", None, &RecognitionOptions::default());
        let record = none["records"][0].as_object().unwrap();
        assert_eq!(record.len(), 2, "{record:?}");
        assert_eq!(
            none["rotate"], true,
            "the endpoint turns the card when needed"
        );
    }

    #[test]
    fn routes_by_category() {
        let sport = json!({"records": [{"_objects": [{"name": "Card", "Top Category": [{"name": "Card"}],
            "_tags": {"Category": [{"name": "Card/Sport Card", "prob": 0.9}]}}], "Graded Slab": [{"name": "no"}]}]});
        assert_eq!(route_from_tags(&sport), (Endpoint::Sport, false));

        let comics = json!({"records": [{"_objects": [{"name": "Comics", "Top Category": [{"name": "Comics"}]}]}]});
        assert_eq!(route_from_tags(&comics), (Endpoint::Comics, false));

        let slab = json!({"records": [{"Graded Slab": [{"name": "yes", "prob": 0.98}], "_objects": [
            {"name": "Slab Label", "area": 0.1}, {"name": "Card", "area": 0.5, "_tags": {"Category": [{"name": "Card/Trading Card Game"}]}}]}]});
        assert_eq!(route_from_tags(&slab), (Endpoint::Tcg, true));

        assert_eq!(route_from_tags(&json!({})), (Endpoint::Tcg, false));
    }

    #[test]
    fn request_flags_follow_the_endpoint() {
        let opts = RecognitionOptions {
            price_stats: true,
            slab: true,
            ..Default::default()
        };
        let tcg = build_request(Endpoint::Tcg, "AAA", None, &opts);
        assert_eq!(tcg["slab_grade"], true);
        assert_eq!(tcg["records"][0]["_base64"], "AAA");
        let comics = build_request(Endpoint::Comics, "AAA", None, &opts);
        assert_eq!(comics["records"][0]["lang"], "en");
        assert!(comics.get("slab_grade").is_none());
        let slab = build_request(Endpoint::Slab, "AAA", None, &opts);
        assert!(slab.get("price_stats").is_none());
    }

    #[test]
    fn the_back_goes_to_sport_and_custom_endpoints() {
        let opts = RecognitionOptions {
            magic_ai: true,
            both_sides: true,
            ..Default::default()
        };
        let sport = build_request(Endpoint::Sport, "F", Some("B"), &opts);
        assert_eq!(sport["magic_ai"], true);
        assert_eq!(sport["records"][1]["_id"], "back");
        assert_eq!(sport["records"][1]["_base64"], "B");
        // TCG backs all look the same; magic_ai is a sport_id option.
        let tcg = build_request(Endpoint::Tcg, "F", Some("B"), &opts);
        assert_eq!(tcg["records"].as_array().unwrap().len(), 1);
        assert!(tcg.get("magic_ai").is_none());
        let custom = RecognitionOptions {
            custom_endpoint: Some("collectibles/v2/acme_id".into()),
            ..opts.clone()
        };
        assert_eq!(custom.path(Endpoint::Tcg), "collectibles/v2/acme_id");
        let tcg = build_request(Endpoint::Tcg, "F", Some("B"), &custom);
        assert_eq!(tcg["records"].as_array().unwrap().len(), 2);
        // without the option, only the front
        let front_only = RecognitionOptions::default();
        let sport = build_request(Endpoint::Sport, "F", Some("B"), &front_only);
        assert_eq!(sport["records"].as_array().unwrap().len(), 1);
        assert!(sport.get("magic_ai").is_none());
    }

    #[test]
    fn custom_paths() {
        assert_eq!(
            custom_path(" /collectibles/v2/acme_id/ ").as_deref(),
            Some("collectibles/v2/acme_id")
        );
        assert_eq!(custom_path(""), None);
        assert_eq!(custom_path("https://example.com/x"), None);
        assert_eq!(custom_path("../account"), None);
    }

    #[test]
    fn the_closer_side_wins() {
        let card = |name: &str, d: f64| {
            json!({"_objects": [{"name": "Card", "_identification": {
                "best_match": {"name": name}, "alternatives": [{"name": "Other"}], "distances": [d, 0.5]}}]})
        };
        let resp = json!({"records": [card("Front read", 0.4), card("Back read", 0.2)]});
        assert_eq!(parse(Endpoint::Sport, &resp).card.name, "Back read");
        let mut picked = resp.clone();
        select_alternative(&mut picked, 0).unwrap();
        assert_eq!(
            picked["records"][1]["_objects"][0]["_identification"]["best_match"]["name"],
            "Other"
        );

        // A back without a match (or with an error status) doesn't hide the front.
        let resp = json!({"records": [card("Front read", 0.4),
            {"_status": {"code": 404}, "_objects": [{"name": "Card", "_identification": {"best_match": null}}]}]});
        assert_eq!(parse(Endpoint::Sport, &resp).card.name, "Front read");
        // Same distance (e.g. magic_ai copies its answer to both sides): the front.
        let resp = json!({"records": [card("Front read", 0.0), card("Back read", 0.0)]});
        assert_eq!(parse(Endpoint::Sport, &resp).card.name, "Front read");
    }

    #[test]
    fn stored_recognitions_know_their_type() {
        let custom = json!({"endpoint": "collectibles/v2/acme_id", "id_type": "sport"});
        assert_eq!(Endpoint::of_raw(&custom), Endpoint::Sport);
        let old = json!({"endpoint": "collectibles/v2/comics_id"});
        assert_eq!(Endpoint::of_raw(&old), Endpoint::Comics);
    }

    #[test]
    fn price_prefers_ungraded_then_graded() {
        let flat = vec![
            json!({"stats_type": "ungraded", "median": 5.0}),
            json!({"stats_type": "graded", "median": 50.0}),
        ];
        assert_eq!(representative_price(&flat), Some(5.0));
        assert_eq!(
            representative_price(&[json!({"stats_type": "overall", "median": -1})]),
            None
        );
    }
}
