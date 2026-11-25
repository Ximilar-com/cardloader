//! The tools agents call, grouped by what Settings → AI agents lets them do. Results are compact JSON (empty
//! fields left out); a card's pictures come as images so the agent can check the match itself.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, GetPromptRequestParams,
    GetPromptResponse, Implementation, JsonObject, ListPromptsResult, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::commands::AppState;
use crate::crop::Quad;
use crate::ebay::listing::CardCondition;
use crate::ebay::sync::DraftItem;
use crate::error::{Error, Result};
use crate::imaging;
use crate::market::{shopify, whatnot, PricedItem};
use crate::model::*;
use crate::runner::{AgentAnswer, AgentTask, SideCorners};
use crate::settings::{McpSettings, Settings};
use crate::source::{self, csv_file};
use crate::ximilar::recognition::{self, ALPHABETS, SPORTS, TCG_GAMES};

/// What the server tells agents up front.
const INSTRUCTIONS: &str = "Cardloader identifies trading cards, sports cards and comics from images with Ximilar's \
API. A session is a folder of images (or a CSV) analysed card by card; each card gets a best match and other \
candidates. Every identified card waits for review until it is accepted. Wherever a session id is asked for, \
\"latest\" means the newest session. Typical work: inspect_folder → create_session → wait_for_session (repeat \
until finished) → list_cards with filter to_review → get_card (look at the picture) → accept_cards or \
select_candidate. To identify the cards yourself instead, create the session with engine \"agent\": Cardloader \
reads the images and hands you each card's pictures through next_cards_to_identify, and you answer with \
identify_cards. Both next_cards_to_identify and next_cards_to_price say whether the user lets you search the web \
(webSearch): when not, use only the cards' pictures and Cardloader's tools. To price identified cards yourself, create the session with prices \"agent\": next_cards_to_price \
hands you each identified card, and you answer with price_cards. A session can give you several of these tasks: \
when a tool's next says cards wait for another one, go on with the tool it names, until each says finished. With prices \"ebay_agent\" Cardloader searches \
eBay for each card and hands you its listings with their pictures: you answer with the listings that are the card \
(price_cards with listing_ids; listing_pictures shows larger pictures), and their median is its price. For what an identified card asks on eBay now, call \
find_ebay_listings, then keep_ebay_listings with the listings that are the card. To crop cards yourself (each \
card cut out of its picture, straightened), create the session with crop_engine \"agent\" (task crop, or \
save_crops): next_cards_to_crop hands you each card's pictures, you find the card's four corners in them, \
preview_crop shows the cut they give, and you answer with crop_cards. Cards you accept are marked as accepted by AI in the app, with your note; the \
user sees everything you do live. Tools you may not use are left out: the user allows them in Cardloader → Settings → AI \
agents.";

/// The long edge of card pictures sent to agents: enough to read the name and number, few tokens.
const PICTURE_LONG_EDGE: u32 = 1024;
/// What the pictures of one tool result may take, as base64: clients cap a result (Claude Desktop refuses one over
/// 1 MB, "Tool result is too large"), and this leaves room for its text.
const PICTURES_BUDGET: usize = 750_000;
/// The least of that budget a card picture gets: about a 1024 px card at a middling JPEG quality. With more
/// pictures than that leaves room for, fewer cards are sent rather than pictures too coarse to read.
const PICTURE_MIN_SHARE: usize = 150_000;
/// Thumbnails of a card's eBay listings sent with it (`ebay_agent`): at most this many, eBay's best matches
/// first, as small as eBay's own search shows them, and within this much of a result, so the card's own pictures
/// keep the rest of `PICTURES_BUDGET` and stay readable. Larger ones come on request (`listing_pictures`).
const LISTING_THUMBS: usize = 24;
const THUMB_EDGE: u32 = 225;
const THUMBS_BUDGET: usize = 250_000;
/// `listing_pictures`: how many listings one call shows, and their long edge unless the agent asks otherwise.
const LISTING_PICTURES: usize = 8;
const LISTING_PICTURE_EDGE: u32 = 800;
/// The long edge of the cut `preview_crop` shows: enough to see whether the card's edges are in it.
const CUT_PREVIEW_LONG_EDGE: u32 = 768;
/// How far the shape of the picture an agent says its corners are in may be from the picture's own (its width
/// over its height): a client may have shrunk the picture, which rounds its sides.
const SHAPE_TOLERANCE: f64 = 0.03;
/// How long `wait_for_session` waits by default, and at most (some clients give up on a call after a minute).
const WAIT_DEFAULT: u64 = 50;
pub(super) const WAIT_MAX: u64 = 120;

#[derive(Clone)]
pub struct Handler {
    state: AppState,
    last_used: Arc<Mutex<Option<String>>>,
}

/// What a tool needs the user to allow; reading is always allowed.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Group {
    Read,
    Run,
    Delete,
    Market,
    Settings,
}

impl Group {
    fn allowed(self, mcp: &McpSettings) -> bool {
        match self {
            Group::Read => true,
            Group::Run => mcp.allow_run,
            Group::Delete => mcp.allow_delete,
            Group::Market => mcp.allow_market,
            Group::Settings => mcp.allow_settings,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Group::Read => "Read",
            Group::Run => "Run & review",
            Group::Delete => "Delete",
            Group::Market => "Marketplaces",
            Group::Settings => "Settings",
        }
    }
}

// ---- arguments ----------------------------------------------------------------------------------

#[derive(Deserialize, JsonSchema)]
struct NoArgs {}

// A session is named by the argument `session`. It was `session_id` (still read, for clients set up with it), a
// name Claude Desktop's own tools use too: called through its relay to this computer, that one argument never
// arrived ("missing field `session_id`"), whatever its value, while every other did.
#[derive(Deserialize, JsonSchema)]
struct SessionArg {
    /// The session: its id, its name, or "latest" for the newest session.
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
}

#[derive(Deserialize, JsonSchema)]
struct ListSessions {
    /// At most this many, newest first (default 20).
    limit: Option<usize>,
    /// Only sessions in this state.
    status: Option<SessionStatus>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
enum CardFilter {
    #[default]
    All,
    /// Identified and not accepted yet.
    ToReview,
    /// Identified with medium or low confidence, or none.
    Uncertain,
    /// Accepted, picked or corrected by an AI agent.
    AcceptedByAi,
    /// Failed to analyse.
    Errors,
    /// Waiting to be analysed.
    Queued,
}

#[derive(Deserialize, JsonSchema)]
struct ListCards {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    #[serde(default)]
    filter: CardFilter,
    /// Skip this many (for paging; default 0).
    offset: Option<usize>,
    /// At most this many (default 50, at most 200).
    limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct GetCard {
    item_id: i64,
    /// Include the front (and back) pictures, about 1024 px (default true).
    images: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct SearchCards {
    /// Text in the name, set, number, location or notes; empty for all.
    #[serde(default)]
    query: String,
    /// review (not accepted yet), graded, duplicates, agent (accepted by AI); empty for all.
    #[serde(default)]
    filter: String,
    /// At most this many, newest first (default 50, at most 200).
    limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct PathArg {
    /// An absolute path (a leading ~ is the home folder).
    path: String,
}

#[derive(Deserialize, JsonSchema)]
struct RemoteSessions {
    /// Page 1 is the newest.
    page: Option<u32>,
    /// Text in the session name.
    search: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct RemoteSession {
    /// The Cards Cloud session id.
    id: String,
}

#[derive(Deserialize, JsonSchema)]
struct CreateSession {
    /// The folder of card images as an absolute path, or the CSV file with `source` csv.
    path: String,
    /// folder (default): analyse the images in the folder; watch_folder: also analyse new images as they arrive,
    /// until stopped; csv: import a CSV of cards or image paths.
    source: Option<SourceKind>,
    /// Default: the folder's name and today's date.
    name: Option<String>,
    /// What identifies the cards: ximilar (the API), local (the model on this computer), or agent: you do, from
    /// their pictures (next_cards_to_identify, then identify_cards). Default: the user's default.
    engine: Option<Engine>,
    /// What grades the cards when they are graded: ximilar or local. Default: as `engine` (ximilar for agent).
    grade_engine: Option<Engine>,
    /// Where identified cards' prices come from: ximilar (price statistics, only when ximilar identifies), ebay
    /// (the median of the card's active eBay listings; needs the user's eBay keyset), ebay_local (the same, of
    /// the listings the local model keeps as the card), ebay_agent (the same, of the listings you keep:
    /// next_cards_to_price hands you them with their pictures, price_cards takes listing_ids), agent (you price
    /// each card: next_cards_to_price, then price_cards), or off. Default: ximilar when `price_stats`, else off.
    prices: Option<Prices>,
    /// For prices from eBay or by you: the eBay site (EBAY_US, EBAY_GB, EBAY_DE, …). Default: the user's.
    ebay_site: Option<String>,
    /// identify: name, set, number, price (and grading if asked); grade: only grade each card (no identification);
    /// condition: only its condition (e.g. Near Mint); crop: only cut each card out of its image, straightened, into
    /// pictures in `crop_folder`. Default: the user's default.
    task: Option<Task>,
    /// Also save each card cropped while identifying or grading it. Default: the user's default. A crop task
    /// always does.
    save_crops: Option<bool>,
    /// What crops the cards: ximilar (default: it cuts, evens out and sharpens them; one more call per card),
    /// local (the model on this computer finds each card's corners), or agent: you find them
    /// (next_cards_to_crop, then crop_cards). Cardloader cuts the card out at the corners found.
    crop_engine: Option<Engine>,
    /// For crops: an absolute folder for the pictures, not the one the cards are read from. Default: the session's
    /// own folder (see cropsFolder in the result).
    crop_folder: Option<String>,
    /// For crops: the file type, jpg (default: the user's last), png (lossless, large) or webp (as Ximilar sends
    /// it; lossless and larger when the card is cut here).
    crop_format: Option<CropFormat>,
    /// What the cards are: tcg (trading card games), sport, comics, slab (graded slabs), or auto (detected per
    /// card). Default: the user's default.
    card_type: Option<IdMode>,
    /// The game or sport of every card, e.g. Pokemon, Magic The Gathering, Baseball. Leave out when mixed.
    game: Option<String>,
    /// The writing system of trading cards: latin (English and other European languages), japanese, chinese,
    /// korean, russian, thai. Leave out when mixed.
    alphabet: Option<String>,
    /// single: one image per card; front_back: images alternate front, back, front, back.
    pairing: Option<Pairing>,
    /// Also send each card's back to identification (sports cards; needs front_back). Default: on for a local-model
    /// session of sport cards, which then pairs front_back.
    read_backs: Option<bool>,
    /// The images are known to be fronts (and backs, when paired): faster and more accurate.
    known_side: Option<bool>,
    /// The images are known to be upright: faster and more accurate.
    upright: Option<bool>,
    /// The images are already cropped to the card.
    cropped: Option<bool>,
    /// Grade the cards' condition after identifying them (billed extra); with task grade, grade or centering.
    grading: Option<GradingMode>,
    /// Mirror the session to Ximilar Cards Cloud ("store it in Ximilar"). Default: the user's default.
    store_in_cards_cloud: Option<bool>,
    /// A Cards Cloud session id or link to continue: files whose cards it has are skipped.
    continue_cards_cloud_session: Option<String>,
    /// Ask Ximilar for market prices (the same as prices ximilar). Default: the user's default.
    price_stats: Option<bool>,
    /// CSV only: also analyse rows that are already identified.
    reanalyse: Option<bool>,
    /// Start analysing right away (default true).
    start: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct RunSession {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Only analyse the cards waiting in the queue: no new files, no scan (default false).
    only_queued: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct NextCards {
    /// A session identified, priced or cropped by an AI agent (its id, its name, or "latest"); empty for any.
    #[serde(rename = "session", alias = "session_id")]
    session_id: Option<String>,
    /// Cards to return at most (default 1, at most 10): each comes with its pictures, front and back, so one card
    /// at a time gets them at their best. Fewer come than asked for when the pictures of that many wouldn't fit
    /// one result: the rest come with the next call.
    max: Option<usize>,
    /// How long to wait for a card while the next ones are scanned (default 50, at most 120 seconds).
    wait_seconds: Option<u64>,
    /// The pictures' long edge in pixels (default 1024; up to 1600 to read small print). A result holds about
    /// 750 KB of pictures: with several cards, or large pictures, they are made coarser or smaller to fit.
    long_edge: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
struct AddToCardsArgs {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Only these cards (item ids); default: every identified card that is finished.
    item_ids: Option<Vec<i64>>,
    /// ebay: the median of each card's active eBay listings (needs the user's eBay keyset); ebay_local: of the
    /// listings the local model keeps as the card; ebay_agent: of the listings you keep (next_cards_to_price
    /// hands you them, price_cards takes listing_ids); agent: you price each card (next_cards_to_price, then
    /// price_cards). Ximilar's prices come only with its identification.
    prices: Option<Prices>,
    /// grade (a full grade), centering, or condition (e.g. Near Mint).
    grading: Option<GradingMode>,
    /// What grades: ximilar or local (the model on this computer). Default: the session's.
    grade_engine: Option<Engine>,
    /// For condition: tcgplayer, cardmarket, ebay_tcg, ebay_sport, or ximilar (Ximilar only). Default: the
    /// session's.
    condition_scale: Option<String>,
    /// For prices: the eBay site (EBAY_US, EBAY_GB, EBAY_DE, …). Default: the session's, else the user's.
    ebay_site: Option<String>,
    /// Save each card cropped: cut out of its picture, straightened.
    crops: Option<bool>,
    /// What crops: ximilar, local (the model on this computer finds the corners) or agent (you find them:
    /// next_cards_to_crop, then crop_cards). Default: the session's, else ximilar.
    crop_engine: Option<Engine>,
    /// For crops: an absolute folder for the pictures. Default: the session's (see cropsFolder in the result).
    crop_folder: Option<String>,
    /// For crops: jpg, png or webp. Default: the session's.
    crop_format: Option<CropFormat>,
}

#[derive(Deserialize, JsonSchema)]
struct CropCards {
    cards: Vec<CardCorners>,
}

#[derive(Deserialize, JsonSchema)]
struct CardCorners {
    item_id: i64,
    /// Where the card is in its front picture. Leave out with uncropped.
    front: Option<PictureCorners>,
    /// Where the card is in its back picture, for a card that came with one.
    back: Option<PictureCorners>,
    /// Instead of `back`: why the back picture has no card to cut out. The front is still cropped.
    no_back: Option<String>,
    /// What you saw, in a few words (shown to the user).
    note: Option<String>,
    /// The front picture has no card to cut out: the card's crop fails with your note.
    uncropped: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct PictureCorners {
    /// The width and height, in pixels, of the picture the corners are in: as given with the picture.
    size: [f64; 2],
    /// The card's four corners as [x, y] pixels from the picture's top-left, clockwise, starting at the card's
    /// own top-left corner (where its top-left is when the card is read upright): a card lying sideways or
    /// upside down then comes out upright.
    corners: [[f64; 2]; 4],
}

#[derive(Deserialize, JsonSchema)]
struct PreviewCrop {
    item_id: i64,
    /// front (default) or back.
    side: Option<String>,
    /// The width and height, in pixels, of the picture the corners are in: as given with the picture.
    size: [f64; 2],
    /// The card's four corners as [x, y] pixels, clockwise from the card's own top-left corner.
    corners: [[f64; 2]; 4],
}

#[derive(Deserialize, JsonSchema)]
struct PriceCards {
    cards: Vec<CardPrice>,
}

#[derive(Deserialize, JsonSchema)]
struct CardPrice {
    item_id: i64,
    /// What the card sells for now (as is: raw, or the slab's grade), in `currency`. Leave out with unpriced.
    price: Option<f64>,
    /// The price's currency (ISO 4217, e.g. EUR for prices from ebay.de). Default: USD.
    currency: Option<String>,
    /// Instead of a price: the ids of the card's eBay listings that are this card, as it is (from
    /// next_cards_to_price in an ebay_agent session, or your own find_ebay_listings). Their median becomes the
    /// card's price. An empty list, with a note, when none of them is the card.
    listing_ids: Option<Vec<String>>,
    /// How you got it, in a few words (shown to the user), e.g. "median of 6 sold listings, no lots".
    note: Option<String>,
    /// You found no price: only the card's price fails, with your note.
    unpriced: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ListingPictures {
    item_id: i64,
    /// Listings of the card's last eBay search (ids from next_cards_to_price or find_ebay_listings): at most 8
    /// a call.
    listing_ids: Vec<String>,
    /// The pictures' long edge in pixels (default 800, 256 to 1024).
    long_edge: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
struct IdentifyCards {
    cards: Vec<CardIdentity>,
}

#[derive(Deserialize, JsonSchema)]
struct CardIdentity {
    item_id: i64,
    /// The name: the player, character or comic title. Leave out with unidentified.
    name: Option<String>,
    /// The set or product, e.g. "2023 Panini Prizm", "Obsidian Flames".
    set_name: Option<String>,
    set_code: Option<String>,
    /// As printed, with its letters, e.g. "RC-12", "125/197".
    card_number: Option<String>,
    year: Option<String>,
    /// The variant, parallel or insert, e.g. "Silver Prizm", "Reverse Holo".
    variant: Option<String>,
    /// Who made the card, e.g. "Topps", "Panini".
    brand: Option<String>,
    /// The player's team on the card.
    team: Option<String>,
    /// The copy's serial number as printed, e.g. "43/75".
    serial: Option<String>,
    /// What a seller would say in a listing title, e.g. "RC Auto". Only what the pictures show.
    features: Option<String>,
    /// The game or sport, e.g. Pokemon, Football, Soccer.
    game: Option<String>,
    /// tcg, sport or comics. Default: from the game, else the session's.
    card_type: Option<String>,
    /// Default: name, set and number.
    full_name: Option<String>,
    /// How sure you are: high, medium or low.
    certainty: Option<String>,
    /// What you read, in a few words (shown to the user).
    note: Option<String>,
    /// You can't tell this card: it fails with your note, and the user can offer it again.
    unidentified: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct FindEbayListings {
    /// An identified card.
    item_id: i64,
    /// Search for this instead of the card's name, set and number (e.g. to add the parallel, or to leave out a
    /// word no listing uses).
    query: Option<String>,
    /// Count auctions too (default false: their current bid is rarely what a card sells for).
    include_auctions: Option<bool>,
    /// Listings to return (default 50, at most 200).
    limit: Option<u32>,
    /// The eBay site: EBAY_US (ebay.com), EBAY_GB, EBAY_DE, EBAY_FR, EBAY_IT, EBAY_ES, EBAY_NL, EBAY_BE, EBAY_AT,
    /// EBAY_IE, EBAY_CH, EBAY_PL, EBAY_CA, EBAY_AU, EBAY_HK or EBAY_SG. Prices are in its currency. Default: the
    /// user's (get_settings).
    marketplace: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct KeepEbayListings {
    item_id: i64,
    /// The listings of the card's last search that are this card; the others (other cards, lots, other grades)
    /// stop counting in its price range.
    listing_ids: Vec<String>,
    /// Why, in a few words (shown to the user), e.g. "left out 3 lots and a PSA 9".
    note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct WaitForSession {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Return after this many seconds even if it still runs (default 50, at most 120).
    timeout_seconds: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
struct AcceptCards {
    item_ids: Vec<i64>,
    /// false takes an accept back (default true).
    accepted: Option<bool>,
    /// Why, in a few words (shown to the user), e.g. "name and 4/102 match the picture".
    note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct SelectCandidate {
    item_id: i64,
    /// The candidate's index, as get_card lists them.
    index: usize,
    /// Why, in a few words (shown to the user).
    note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct EditCard {
    item_id: i64,
    name: Option<String>,
    full_name: Option<String>,
    set_name: Option<String>,
    set_code: Option<String>,
    card_number: Option<String>,
    series: Option<String>,
    year: Option<String>,
    /// The game or sport.
    subcategory: Option<String>,
    brand: Option<String>,
    team: Option<String>,
    /// The copy's serial number, e.g. "43/75".
    serial: Option<String>,
    /// What a listing title says about the card, e.g. "RC Auto".
    features: Option<String>,
    /// The card's own listing title (80 characters at most), instead of the one its template makes; "" goes
    /// back to the automatic one.
    title: Option<String>,
    price: Option<f64>,
    /// The user's notes on the card (local only).
    notes: Option<String>,
    /// Where the card is kept, e.g. "Box 3" (local only).
    location: Option<String>,
    /// Why, in a few words (shown to the user).
    note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct RetryCards {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Default: every card that failed.
    item_ids: Option<Vec<i64>>,
}

#[derive(Deserialize, JsonSchema)]
struct ItemIds {
    item_ids: Vec<i64>,
}

#[derive(Deserialize, JsonSchema)]
struct EditCards {
    item_ids: Vec<i64>,
    /// What the text goes to: title (each card's own listing title), features, brand, team or series.
    field: String,
    /// start, end (the default) or replace. replace with an empty text empties the field; a title is then the
    /// automatic one again.
    place: Option<String>,
    text: String,
    /// true: change nothing, only return the titles the change would give.
    preview: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct SetLocation {
    item_ids: Vec<i64>,
    /// e.g. "Box 3, row 2"; empty clears it.
    location: String,
}

#[derive(Deserialize, JsonSchema)]
struct ExportCsv {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Where to write the file: an absolute path ending in .csv, outside Cardloader's own folders.
    path: String,
    /// Replace the file if it exists (default false).
    overwrite: Option<bool>,
    /// The columns, in order (default: the ones the user last chose in the app, else all of them).
    columns: Option<Vec<crate::export::CsvColumn>>,
    /// Headers to write instead of the columns' own, by column, e.g. {"name": "primary name"} (default: the ones
    /// the user last wrote in the app).
    headers: Option<crate::export::Headers>,
}

#[derive(Deserialize, JsonSchema)]
struct ExportTcgplayer {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Where to write the file: an absolute path ending in .csv, outside Cardloader's own folders.
    path: String,
    /// For cards whose condition wasn't graded (default near_mint).
    condition: Option<CardCondition>,
    /// The cards and their prices. Default: every identified card at its market price.
    items: Option<Vec<PricedItem>>,
    overwrite: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct DeleteCards {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    item_ids: Vec<i64>,
}

#[derive(Deserialize, JsonSchema)]
struct DeleteSession {
    /// The session id ("latest" is not accepted here).
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Also delete its copy in Cards Cloud (default false).
    also_in_cards_cloud: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct EbayDrafts {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// The cards and their prices. Default: every identified card that has a market price, at that price.
    items: Option<Vec<DraftItem>>,
    /// For cards whose condition wasn't graded (default near_mint).
    condition: Option<CardCondition>,
}

#[derive(Deserialize, JsonSchema)]
struct EbayCards {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Default: all of the session's cards that apply.
    item_ids: Option<Vec<i64>>,
}

#[derive(Deserialize, JsonSchema)]
struct EbayRefresh {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Read the listings even if they were read a few minutes ago.
    force: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ExportWhatnot {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Where to write the file: an absolute path ending in .csv, outside Cardloader's own folders.
    path: String,
    options: whatnot::Options,
    /// Default: every identified card at its market price.
    items: Option<Vec<PricedItem>>,
    overwrite: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ExportShopify {
    /// The session: its id, its name, or "latest".
    #[serde(rename = "session", alias = "session_id")]
    session_id: String,
    /// Where to write the file: an absolute path ending in .csv, outside Cardloader's own folders.
    path: String,
    options: shopify::Options,
    /// Default: every identified card at its market price.
    items: Option<Vec<PricedItem>>,
    overwrite: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateDefaults {
    /// What new sessions do: identify, or only grade, condition or crop.
    task: Option<Task>,
    /// New sessions that identify or grade also save the cards cropped.
    save_crops: Option<bool>,
    card_type: Option<IdMode>,
    /// The game or sport new sessions start with; empty for any.
    game: Option<String>,
    grading: Option<GradingMode>,
    pairing: Option<Pairing>,
    /// tcgplayer, cardmarket, ebay_tcg or ebay_sport.
    condition_scale: Option<String>,
    store_in_cards_cloud: Option<bool>,
    price_stats: Option<bool>,
    /// Recognition requests at once (1–3).
    concurrency: Option<u32>,
    /// The folder new sessions start in.
    folder: Option<String>,
}

// ---- the list of tools --------------------------------------------------------------------------

fn schema<A: JsonSchema>() -> Arc<JsonObject> {
    let settings = schemars::generate::SchemaSettings::draft2020_12().with(|s| {
        s.inline_subschemas = true;
    });
    let mut value = settings
        .into_generator()
        .into_root_schema_for::<A>()
        .to_value();
    let object = value
        .as_object_mut()
        .expect("a struct's schema is an object");
    object.remove("$schema");
    object.remove("title");
    // Names the user's taxonomy spells exactly; schemars can't take them from the constants.
    if let Some(game) = object.get_mut("properties").and_then(|p| p.get_mut("game")) {
        game["examples"] = json!(TCG_GAMES.iter().chain(SPORTS).collect::<Vec<_>>());
    }
    Arc::new(object.clone())
}

struct Spec {
    group: Group,
    tool: Tool,
}

fn spec<A: JsonSchema>(
    group: Group,
    name: &'static str,
    title: &str,
    description: &'static str,
) -> Spec {
    let annotations = ToolAnnotations::with_title(title)
        .read_only(group == Group::Read)
        .destructive(group == Group::Delete)
        .open_world(group == Group::Market);
    Spec {
        group,
        tool: Tool::new(name, description, schema::<A>()).annotate(annotations),
    }
}

fn specs() -> Vec<Spec> {
    use Group::*;
    vec![
        spec::<NoArgs>(Read, "get_status", "Status", "Whether the Ximilar account and Cards Cloud are reachable, the defaults new sessions get, running sessions, cards waiting for review, and what you are allowed to do."),
        spec::<ListSessions>(Read, "list_sessions", "List sessions", "Sessions, newest first, with their counts (identified, to review, errors, queued) and state."),
        spec::<SessionArg>(Read, "get_session", "Get a session", "One session with its counts and options."),
        spec::<ListCards>(Read, "list_cards", "List cards", "A session's cards in order, filtered and paged: the match, confidence, price and whether it is accepted."),
        spec::<GetCard>(Read, "get_card", "Get a card", "One card: its match, the other candidates (by index), price, grade, notes, and its front and back pictures. Compare the printed name, number and set with the match before accepting it."),
        spec::<SearchCards>(Read, "search_cards", "Search cards", "Identified cards of every session, newest first, by text."),
        spec::<PathArg>(Read, "inspect_folder", "Inspect a folder", "What a folder holds before analysing it: how many images, some file names, sessions already using it. For a CSV file: what importing it would bring in."),
        spec::<RemoteSessions>(Read, "list_cards_cloud_sessions", "List Cards Cloud sessions", "Sessions stored in Ximilar Cards Cloud (also those made by other apps)."),
        spec::<RemoteSession>(Read, "get_cards_cloud_session", "Get a Cards Cloud session", "A Cards Cloud session with its summary."),
        spec::<NoArgs>(Read, "get_settings", "Get settings", "The defaults new sessions get and what agents may do."),
        spec::<CreateSession>(Run, "create_session", "Create a session", "Creates a session for a folder of card images (or a CSV) and starts analysing it. Unset options take the user's defaults. Then call wait_for_session."),
        spec::<RunSession>(Run, "run_session", "Run a session", "Starts or resumes a session; with only_queued, analyses just the cards waiting in its queue."),
        spec::<SessionArg>(Run, "pause_session", "Pause a session", "Pauses a running session; run_session resumes it."),
        spec::<SessionArg>(Run, "stop_session", "Stop a session", "Stops a running session."),
        spec::<NextCards>(Run, "next_cards_to_identify", "Cards to identify", "Cards of sessions with engine agent that wait for you to identify them, each with its front (and back) picture. Waits while the next cards are scanned; says finished when no more can come. Answer with identify_cards."),
        spec::<IdentifyCards>(Run, "identify_cards", "Identify cards", "Your identification of cards from next_cards_to_identify: name, set, number, year, variant, game. The session then goes on with them (grading, crops, Cards Cloud) and the user reviews them."),
        spec::<AddToCardsArgs>(Run, "add_to_cards", "Add prices, grading or crops", "Adds prices (from eBay's listings, or priced by you), grading (a grade, centering or the condition, by Ximilar or the local model) or crops (by Ximilar, the local model, or with corners you find) to a finished session's identified cards, without identifying them again, and runs them. The session keeps the task for cards it gets later."),
        spec::<NextCards>(Run, "next_cards_to_crop", "Cards to crop", "Cards of sessions with crop_engine agent that wait for you to find them in their pictures: each card's front (and back) picture with its size in pixels. Waits while the next cards are scanned; says finished when no more can come. Answer with crop_cards."),
        spec::<PreviewCrop>(Run, "preview_crop", "Preview a crop", "Shows the picture that four corners would cut out of one side of a card, without saving anything: check that the whole card is in it and nothing else, correct the corners, then send them with crop_cards."),
        spec::<CropCards>(Run, "crop_cards", "Crop cards", "Where the cards from next_cards_to_crop are in their pictures: each side's four corners in the picture's pixels. Cardloader cuts each card out of its full scan, straightened, into the session's crop folder."),
        spec::<NextCards>(Run, "next_cards_to_price", "Cards to price", "Identified cards of sessions with prices agent or ebay_agent that wait for you to price them: their name, set, number, year, variant, game, slab grade, any eBay range already found, and their front (and back) picture. In an ebay_agent session each card comes alone with the eBay listings Cardloader found for it and small pictures of them: you say which are the card. Waits while the next cards are identified; says finished when no more can come. Answer with price_cards."),
        spec::<PriceCards>(Run, "price_cards", "Price cards", "Your price of cards from next_cards_to_price (in US dollars unless you give each one's currency), with a note on how you got it; or, instead of a price, listing_ids: the card's eBay listings that are this card, whose median becomes its price (what an ebay_agent session asks for). find_ebay_listings and keep_ebay_listings can help when the user has an eBay keyset."),
        spec::<ListingPictures>(Run, "listing_pictures", "Pictures of eBay listings", "Larger pictures of some of a card's eBay listings (from its last search), to compare with the card's own pictures before you say which listings are the card: the same card, parallel and condition can be hard to tell from a title."),
        spec::<FindEbayListings>(Run, "find_ebay_listings", "Find eBay listings", "Searches eBay for an identified card's active fixed-price listings (by its name, set and number, or your query) and keeps them on the card with their price range: min, max, average and median. Then call keep_ebay_listings with the listings that are this card. Needs the user's eBay keyset."),
        spec::<KeepEbayListings>(Run, "keep_ebay_listings", "Keep eBay listings", "Of a card's last eBay search, only the listings that are this card count in its price range (the user sees the others left out, with your note)."),
        spec::<WaitForSession>(Run, "wait_for_session", "Wait for a session", "Waits until the session's run ends or the timeout passes, and returns its progress. Call it again while it still runs."),
        spec::<AcceptCards>(Run, "accept_cards", "Accept cards", "Accepts the cards' matches (or takes that back with accepted false). They are marked as accepted by AI with your note. Cards the user accepted stay theirs."),
        spec::<SelectCandidate>(Run, "select_candidate", "Select a candidate", "Makes another candidate the card's match (get_card lists them by index); the card counts as accepted."),
        spec::<EditCard>(Run, "edit_card", "Edit a card", "Corrects a card's fields or price by hand, or sets its notes, location, details (brand, team, serial, features) or its own listing title; given fields change, others stay. Changed card fields drop the eBay listings Cardloader searched for the card as it was: correct a card before you choose its listings."),
        spec::<EditCards>(Run, "edit_cards", "Edit many cards", "Adds one text to many cards at once: to the start or end of their listing titles (the title is fitted into eBay's 80 characters around it and becomes the card's own), or to their features, brand, team or series. Returns each card's listing title and its length; with preview true nothing is changed. Cards that can't take it are skipped with the reason."),
        spec::<RetryCards>(Run, "retry_cards", "Analyse cards again", "Puts the failed cards (or the given ones) back in the queue and analyses them."),
        spec::<ItemIds>(Run, "turn_cards_upright", "Turn cards upright", "Turns the pictures (front and back) of the cards Ximilar found upside down; other cards are left as they are."),
        spec::<SetLocation>(Run, "set_location", "Set where cards are kept", "Sets where the cards are kept (a box, a binder page)."),
        spec::<SessionArg>(Run, "sync_session", "Sync to Cards Cloud", "Sends the session's changes to Cards Cloud now."),
        spec::<ExportCsv>(Run, "export_csv", "Export a CSV", "Writes the session's cards to a CSV file."),
        spec::<ExportTcgplayer>(Run, "export_tcgplayer", "Export for TCGplayer", "Writes the TCGplayer list (ungraded trading cards) for Seller Portal."),
        spec::<DeleteCards>(Delete, "delete_cards", "Delete cards", "Deletes cards from a stopped session; analysing the folder again doesn't bring them back."),
        spec::<DeleteSession>(Delete, "delete_session", "Delete a session", "Deletes a session and the files Cardloader made for it (never the user's images), optionally its Cards Cloud copy."),
        spec::<RemoteSession>(Delete, "delete_cards_cloud_session", "Delete a Cards Cloud session", "Deletes a session in Cards Cloud; local sessions linked to it keep their cards."),
        spec::<SessionArg>(Market, "ebay_preview", "Preview eBay listings", "How each identified card of the session would be listed on eBay."),
        spec::<EbayDrafts>(Market, "ebay_create_drafts", "Create eBay drafts", "Creates unpublished eBay offers (drafts) for the cards at the given prices. The job runs in the background: see ebay_status."),
        spec::<EbayCards>(Market, "ebay_publish", "Publish on eBay", "Publishes the session's eBay drafts (or the given cards'): they go live."),
        spec::<EbayRefresh>(Market, "ebay_refresh", "Refresh eBay listings", "Reads the session's listings back from eBay (sold, ended)."),
        spec::<DeleteCards>(Market, "ebay_remove", "Remove from eBay", "Takes the cards off eBay: drafts are deleted, live listings end."),
        spec::<SessionArg>(Market, "ebay_status", "eBay job status", "The session's running eBay job, if any."),
        spec::<ExportWhatnot>(Market, "export_whatnot", "Export for Whatnot", "Uploads the pictures (ImgBB) and writes Whatnot's bulk listing CSV."),
        spec::<ExportShopify>(Market, "export_shopify", "Export for Shopify", "Uploads the pictures (ImgBB) and writes Shopify's product CSV, or its inventory CSV."),
        spec::<UpdateDefaults>(Settings, "update_defaults", "Change defaults", "Changes the defaults new sessions get; given fields change, others stay."),
    ]
}

// ---- the server ---------------------------------------------------------------------------------

impl ServerHandler for Handler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(
            Implementation::new("cardloader", env!("CARGO_PKG_VERSION")).with_title("Cardloader"),
        )
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, ErrorData> {
        let mcp = self.state.settings.get().mcp;
        Ok(ListToolsResult::with_all_items(
            specs()
                .into_iter()
                .filter(|s| s.group.allowed(&mcp))
                .map(|s| s.tool)
                .collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResponse, ErrorData> {
        let name = request.name.as_ref();
        let Some(group) = specs()
            .into_iter()
            .find(|s| s.tool.name == name)
            .map(|s| s.group)
        else {
            return Err(ErrorData::invalid_params(
                format!("no tool named {name}"),
                None,
            ));
        };
        if !group.allowed(&self.state.settings.get().mcp) {
            return Ok(failure(format!(
                "Not allowed: the user can allow \"{}\" in Cardloader → Settings → AI agents.",
                group.label()
            )));
        }
        *self.last_used.lock().unwrap() = Some(chrono::Utc::now().to_rfc3339());
        let args = Value::Object(request.arguments.unwrap_or_default());
        Ok(match self.call(name, args).await {
            Ok(content) => CallToolResult::success(content).into(),
            Err(e) => failure(e.to_string()),
        })
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult::with_all_items(super::prompts::list()))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<GetPromptResponse, ErrorData> {
        super::prompts::get(&request.name, request.arguments.as_ref())
            .map(Into::into)
            .ok_or_else(|| {
                ErrorData::invalid_params(format!("no prompt named {}", request.name), None)
            })
    }
}

fn failure(message: impl Into<String>) -> CallToolResponse {
    CallToolResult::error(vec![ContentBlock::text(message)]).into()
}

fn args<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| Error::msg(format!("invalid arguments: {e}")))
}

fn out(value: Value) -> Result<Vec<ContentBlock>> {
    Ok(vec![ContentBlock::text(compact(value).to_string())])
}

/// Leaves out nulls and empty strings, lists and objects: fewer tokens, same meaning.
fn compact(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, compact(v)))
                .filter(|(_, v)| !is_empty(v))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(compact).collect()),
        v => v,
    }
}

fn is_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

impl Handler {
    pub fn new(state: AppState, last_used: Arc<Mutex<Option<String>>>) -> Self {
        Handler { state, last_used }
    }

    async fn call(&self, name: &str, a: Value) -> Result<Vec<ContentBlock>> {
        match name {
            "get_status" => self.status().await,
            "list_sessions" => self.list_sessions(args(a)?),
            "get_session" => {
                let a: SessionArg = args(a)?;
                let session = self.state.db.session(&self.session_id(&a.session_id)?)?;
                out(self.session_json(&session, true))
            }
            "list_cards" => self.list_cards(args(a)?),
            "get_card" => {
                let a: GetCard = args(a)?;
                self.card(a.item_id, a.images.unwrap_or(true)).await
            }
            "search_cards" => self.search_cards(args(a)?),
            "inspect_folder" => self.inspect(args(a)?).await,
            "list_cards_cloud_sessions" => {
                let a: RemoteSessions = args(a)?;
                let api = self.state.cards_cloud()?;
                out(api
                    .list_sessions(
                        a.page.unwrap_or(1).max(1),
                        a.search.as_deref().unwrap_or(""),
                    )
                    .await?)
            }
            "get_cards_cloud_session" => {
                let a: RemoteSession = args(a)?;
                out(self.state.remote_session(a.id.trim()).await?)
            }
            "get_settings" => {
                let s = self.state.settings.get();
                out(json!({"defaults": defaults(&s), "agentsMay": allowed(&s.mcp)}))
            }
            "create_session" => self.create_session(args(a)?).await,
            "run_session" => {
                let a: RunSession = args(a)?;
                let id = self.session_id(&a.session_id)?;
                if a.only_queued.unwrap_or(false) {
                    self.state.runner.analyse_queued(&id)?;
                } else {
                    self.state.runner.start(&id)?;
                }
                self.session_out(&id)
            }
            "pause_session" => {
                let id = self.session_id(&args::<SessionArg>(a)?.session_id)?;
                self.state.runner.pause(&id)?;
                self.session_out(&id)
            }
            "stop_session" => {
                let id = self.session_id(&args::<SessionArg>(a)?.session_id)?;
                self.state.runner.stop(&id)?;
                self.session_out(&id)
            }
            "wait_for_session" => self.wait(args(a)?).await,
            "next_cards_to_identify" => self.next_cards(args(a)?, AgentTask::Identify).await,
            "identify_cards" => self.identify_cards(args(a)?),
            "next_cards_to_price" => self.next_cards(args(a)?, AgentTask::Price).await,
            "price_cards" => self.price_cards(args(a)?),
            "next_cards_to_crop" => self.next_cards(args(a)?, AgentTask::Crop).await,
            "preview_crop" => self.preview_crop(args(a)?).await,
            "crop_cards" => self.crop_cards(args(a)?),
            "add_to_cards" => {
                let a: AddToCardsArgs = args(a)?;
                let id = self.session_id(&a.session_id)?;
                let crops = a.crops.unwrap_or(false);
                let add = AddToCards {
                    prices: a.prices,
                    grading: a.grading,
                    grade_engine: a.grade_engine,
                    condition_mode: a.condition_scale,
                    local_model: None,
                    ebay_site: a.ebay_site,
                    crops,
                    crop_engine: a.crop_engine,
                    crop_folder: match a.crop_folder.as_deref().map(str::trim) {
                        Some(folder) if crops && !folder.is_empty() => {
                            Some(self.crop_folder(folder)?)
                        }
                        _ => None,
                    },
                    crop_format: a.crop_format,
                    crop_level: None,
                };
                let n = self.state.add_to_cards(&id, a.item_ids.as_deref(), &add)?;
                let session = self.state.db.session(&id)?;
                let mut v = self.session_json(&session, false);
                v["cardsAdded"] = json!(n);
                if crops {
                    v["cropsFolder"] = json!(self.state.runner.crop_dir(&session));
                }
                let by_agent = SessionOptions {
                    // Only what was added is the agent's to do now: the cards are identified already.
                    engine: Engine::Ximilar,
                    prices: a
                        .prices
                        .filter(|p| matches!(p, Prices::Agent | Prices::EbayAgent)),
                    save_crops: crops,
                    ..session.options.clone()
                };
                v["next"] = json!(if n == 0 {
                    "no finished identified card to add to".to_string()
                } else {
                    next_for(&by_agent)
                });
                out(v)
            }
            "find_ebay_listings" => self.find_ebay_listings(args(a)?).await,
            "keep_ebay_listings" => self.keep_ebay_listings(args(a)?),
            "listing_pictures" => self.listing_pictures(args(a)?).await,
            "accept_cards" => self.accept(args(a)?),
            "select_candidate" => {
                let a: SelectCandidate = args(a)?;
                let by = agent(a.note);
                self.state.runner.pick_match(a.item_id, a.index, &by)?;
                self.card(a.item_id, false).await
            }
            "edit_card" => self.edit(args(a)?).await,
            "edit_cards" => self.edit_many(args(a)?),
            "retry_cards" => {
                let a: RetryCards = args(a)?;
                let id = self.session_id(&a.session_id)?;
                let n = self.state.retry_items(&id, a.item_ids.as_deref())?;
                if n > 0 && !self.state.runner.is_running(&id) {
                    self.state.runner.analyse_queued(&id)?;
                }
                out(
                    json!({"queued": n, "session": self.session_json(&self.state.db.session(&id)?, false)}),
                )
            }
            "turn_cards_upright" => {
                let a: ItemIds = args(a)?;
                let (turned, skipped) = self.state.runner.turn_items(&a.item_ids)?;
                out(json!({"turned": turned, "notUpsideDown": skipped}))
            }
            "set_location" => {
                let a: SetLocation = args(a)?;
                let mut n = 0;
                for (session, ids) in self.by_session(&a.item_ids)? {
                    n += self
                        .state
                        .runner
                        .locate_items(&session, &ids, &a.location)?;
                }
                out(json!({"changed": n}))
            }
            "sync_session" => {
                let id = self.session_id(&args::<SessionArg>(a)?.session_id)?;
                self.state.runner.sync_now(&id)?;
                self.session_out(&id)
            }
            "export_csv" => {
                let a: ExportCsv = args(a)?;
                let id = self.session_id(&a.session_id)?;
                let path = self.export_path(&a.path, a.overwrite)?;
                let cards = self
                    .state
                    .export_csv(&id, &path, a.columns, a.headers, false)?;
                out(json!({"path": path, "cards": cards}))
            }
            "export_tcgplayer" => {
                let a: ExportTcgplayer = args(a)?;
                let id = self.session_id(&a.session_id)?;
                let path = self.export_path(&a.path, a.overwrite)?;
                let items = self.priced(&id, a.items)?;
                let export = self.state.export_tcgplayer(
                    &id,
                    &items,
                    a.condition.unwrap_or_default(),
                    &path.to_string_lossy(),
                )?;
                out(json!({"path": path, "export": export}))
            }
            "delete_cards" => {
                let a: DeleteCards = args(a)?;
                let id = self.session_id(&a.session_id)?;
                out(json!({"deleted": self.state.runner.delete_items(&id, &a.item_ids)?}))
            }
            "delete_session" => {
                let a: DeleteSession = args(a)?;
                if a.session_id.trim().eq_ignore_ascii_case("latest") {
                    return Err(Error::msg("name the session to delete by its id"));
                }
                self.state
                    .delete_session(a.session_id.trim(), a.also_in_cards_cloud.unwrap_or(false))
                    .await?;
                out(json!({"deleted": a.session_id.trim()}))
            }
            "delete_cards_cloud_session" => {
                let a: RemoteSession = args(a)?;
                self.state.delete_remote_session(a.id.trim()).await?;
                out(json!({"deleted": a.id.trim()}))
            }
            "ebay_preview" => {
                let id = self.session_id(&args::<SessionArg>(a)?.session_id)?;
                out(serde_json::to_value(self.state.ebay_preview(&id)?)?)
            }
            "ebay_create_drafts" => {
                let a: EbayDrafts = args(a)?;
                let id = self.session_id(&a.session_id)?;
                let items = match a.items {
                    Some(items) => items,
                    None => self
                        .identified(&id)?
                        .filter_map(|i| {
                            i.usd_price().filter(|p| *p > 0.0).map(|price| DraftItem {
                                item_id: i.id,
                                price,
                            })
                        })
                        .collect(),
                };
                self.state
                    .ebay_create_drafts(&id, items, a.condition.unwrap_or_default())?;
                out(json!({"started": true, "job": self.state.runner.ebay_job(&id)}))
            }
            "ebay_publish" => {
                let a: EbayCards = args(a)?;
                let id = self.session_id(&a.session_id)?;
                self.state.ebay_ready()?;
                self.state
                    .runner
                    .ebay_start(&id, crate::ebay::sync::Job::Publish(a.item_ids))?;
                out(json!({"started": true, "job": self.state.runner.ebay_job(&id)}))
            }
            "ebay_refresh" => {
                let a: EbayRefresh = args(a)?;
                let id = self.session_id(&a.session_id)?;
                self.state.ebay_refresh(&id, a.force.unwrap_or(false))?;
                out(json!({"started": true, "job": self.state.runner.ebay_job(&id)}))
            }
            "ebay_remove" => {
                let a: DeleteCards = args(a)?;
                let id = self.session_id(&a.session_id)?;
                self.state
                    .runner
                    .ebay_start(&id, crate::ebay::sync::Job::Remove(a.item_ids))?;
                out(json!({"started": true, "job": self.state.runner.ebay_job(&id)}))
            }
            "ebay_status" => {
                let id = self.session_id(&args::<SessionArg>(a)?.session_id)?;
                let session = self.state.db.session(&id)?;
                out(json!({
                    "job": self.state.runner.ebay_job(&id),
                    "error": session.ebay_error,
                    "drafts": session.counts.ebay_drafts,
                    "listed": session.counts.ebay_listed,
                    "sold": session.counts.ebay_sold,
                }))
            }
            "export_whatnot" => {
                let a: ExportWhatnot = args(a)?;
                let id = self.session_id(&a.session_id)?;
                let path = self.export_path(&a.path, a.overwrite)?;
                let items = self.priced(&id, a.items)?;
                let export = self
                    .state
                    .export_whatnot(&id, &items, &a.options, &path.to_string_lossy(), |_, _| {})
                    .await?;
                out(json!({"path": path, "export": export}))
            }
            "export_shopify" => {
                let a: ExportShopify = args(a)?;
                let id = self.session_id(&a.session_id)?;
                let path = self.export_path(&a.path, a.overwrite)?;
                let items = self.priced(&id, a.items)?;
                let export = self
                    .state
                    .export_shopify(&id, &items, &a.options, &path.to_string_lossy(), |_, _| {})
                    .await?;
                out(json!({"path": path, "export": export}))
            }
            "update_defaults" => self.update_defaults(args(a)?),
            _ => Err(Error::msg(format!("no tool named {name}"))),
        }
    }

    /// The session an agent named: its id, or "latest" (also empty) for the newest.
    /// The session an agent means: its id (also inside other text, e.g. "(id …)"), its name, or "latest". Agents
    /// pass what the user told them, often the name; a name several sessions share is refused with their ids.
    fn session_id(&self, given: &str) -> Result<String> {
        let given = given
            .trim()
            .trim_matches(|c: char| "\"'“”„«»「」".contains(c))
            .trim();
        let sessions = || self.state.db.sessions();
        if given.is_empty() || given.eq_ignore_ascii_case("latest") {
            return sessions()?
                .into_iter()
                .next()
                .map(|s| s.id)
                .ok_or_else(|| Error::msg("there are no sessions yet"));
        }
        if let Ok(s) = self.state.db.session(given) {
            return Ok(s.id);
        }
        if let Some(id) = uuid_in(given) {
            return Ok(self.state.db.session(id)?.id);
        }
        let named: Vec<Session> = sessions()?
            .into_iter()
            .filter(|s| s.name.trim().eq_ignore_ascii_case(given))
            .collect();
        match named.as_slice() {
            [one] => Ok(one.id.clone()),
            [] => Err(Error::msg(format!(
                "no session has the id or name \"{given}\": list_sessions shows them"
            ))),
            many => Err(Error::msg(format!(
                "{} sessions are named \"{given}\": pass one's id ({})",
                many.len(),
                many.iter()
                    .map(|s| s.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }

    fn session_out(&self, id: &str) -> Result<Vec<ContentBlock>> {
        out(self.session_json(&self.state.db.session(id)?, false))
    }

    fn session_json(&self, s: &Session, options: bool) -> Value {
        let c = &s.counts;
        let mut v = json!({
            "id": s.id,
            "name": s.name,
            "createdAt": s.created_at,
            "status": s.status,
            "running": self.state.runner.is_running(&s.id),
            "source": s.source_kind,
            "path": s.source_ref,
            "task": s.options.task,
            "engine": s.options.engine,
            "gradeEngine": s.options.grade_engine(),
            "prices": s.options.prices(),
            "cropEngine": s.options.crops().then(|| s.options.crop_engine()),
            "cardType": s.id_mode,
            "game": s.options.subcategory,
            "storeInCardsCloud": s.options.sync_remote,
            "cardsCloudSession": s.remote_session_id,
            "error": s.error,
            "syncError": s.sync_error,
            "counts": {
                "total": c.total,
                "identified": c.done + c.grading,
                "toReview": c.to_review,
                "errors": c.errors,
                "queued": c.queued,
                "inProgress": c.in_progress,
                "grading": c.grading,
                "synced": c.synced,
                "value": (c.total_price * 100.0).round() / 100.0,
                "values": c.values,
                "apiCalls": c.calls,
            },
        });
        if s.options.agent() {
            v["counts"]["waitingForAgent"] = json!(self
                .state
                .runner
                .agent_waiting(Some(&s.id), AgentTask::Identify)
                .len());
        }
        if s.options.prices_by_agent() {
            v["counts"]["waitingForPrice"] = json!(self
                .state
                .runner
                .agent_waiting(Some(&s.id), AgentTask::Price)
                .len());
        }
        if s.options.crops_by_agent() {
            v["counts"]["waitingForCrop"] = json!(self
                .state
                .runner
                .agent_waiting(Some(&s.id), AgentTask::Crop)
                .len());
        }
        if options {
            v["options"] = serde_json::to_value(&s.options).unwrap_or(Value::Null);
        }
        v
    }

    async fn status(&self) -> Result<Vec<ContentBlock>> {
        let settings = self.state.settings.get();
        let connection = match self.state.connection().await {
            Ok(c) => serde_json::to_value(c)?,
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        };
        let sessions = self.state.db.sessions()?;
        let running: Vec<Value> = sessions
            .iter()
            .filter(|s| self.state.runner.is_running(&s.id))
            .map(|s| json!({"id": s.id, "name": s.name}))
            .collect();
        out(json!({
            "app": env!("CARGO_PKG_VERSION"),
            "ximilar": connection,
            "sessions": sessions.len(),
            "latestSession": sessions.first().map(|s| json!({"id": s.id, "name": s.name})),
            "running": running,
            "cardsToReview": sessions.iter().map(|s| s.counts.to_review).sum::<i64>(),
            "cardsWaitingForAgent": self.state.runner.agent_waiting_count(None),
            "defaults": defaults(&settings),
            "agentsMay": allowed(&settings.mcp),
        }))
    }

    fn list_sessions(&self, a: ListSessions) -> Result<Vec<ContentBlock>> {
        let sessions: Vec<Value> = self
            .state
            .db
            .sessions()?
            .iter()
            .filter(|s| a.status.is_none_or(|status| s.status == status))
            .take(a.limit.unwrap_or(20).clamp(1, 200))
            .map(|s| self.session_json(s, false))
            .collect();
        out(json!({ "sessions": sessions }))
    }

    fn list_cards(&self, a: ListCards) -> Result<Vec<ContentBlock>> {
        let id = self.session_id(&a.session_id)?;
        let session = self.state.db.session(&id)?;
        let matching: Vec<ItemSummary> = self
            .state
            .db
            .items(&id)?
            .into_iter()
            .filter(|i| keeps(a.filter, i))
            .collect();
        let offset = a.offset.unwrap_or(0);
        let limit = a.limit.unwrap_or(50).clamp(1, 200);
        let cards: Vec<Value> = matching
            .iter()
            .skip(offset)
            .take(limit)
            .map(card_row)
            .collect();
        let shown = cards.len();
        out(json!({
            "session": {"id": session.id, "name": session.name},
            "matching": matching.len(),
            "offset": offset,
            "cards": cards,
            "more": offset + shown < matching.len(),
        }))
    }

    async fn card(&self, item_id: i64, images: bool) -> Result<Vec<ContentBlock>> {
        let item = self.state.db.item(item_id)?;
        let response = item
            .recognition
            .as_ref()
            .map(|r| &r["response"])
            .unwrap_or(&Value::Null);
        let endpoint = item.recognition.as_ref().map(recognition::Endpoint::of_raw);
        let mut v = card_row(&item.summary);
        v["sessionId"] = json!(item.summary.session_id);
        v["notes"] = json!(item.summary.notes);
        // What it would be listed as; `customTitle` is set when someone wrote it for this card.
        let titles = self.state.settings.get().titles;
        v["listingTitle"] = json!(crate::ebay::listing::plan(&item.summary, &titles)
            .ok()
            .map(|p| p.title));
        v["candidates"] = json!(recognition::candidates(response));
        v["rotation"] = json!(recognition::rotation(response));
        v["links"] = json!(endpoint.map(|e| recognition::parse(e, response).links));
        v["files"] = json!({"front": item.summary.front_path, "back": item.summary.back_path});
        v["analyzedAt"] = json!(item.analyzed_at);
        let mut content = out(v)?;
        if images {
            let mut shown = vec![pictures(&item, PICTURE_LONG_EDGE).await?];
            fit(&mut shown, PICTURE_LONG_EDGE, PICTURES_BUDGET).await?;
            content.extend(blocks(&shown[0]).0);
        }
        Ok(content)
    }

    /// The cards waiting for the agent to identify, price or crop them (`task`), with their pictures; long-polls
    /// while a session that may still offer some runs.
    async fn next_cards(&self, a: NextCards, task: AgentTask) -> Result<Vec<ContentBlock>> {
        let offers = |o: &SessionOptions| match task {
            AgentTask::Identify => o.agent(),
            AgentTask::Price => o.prices_by_agent(),
            AgentTask::Crop => o.crops_by_agent(),
        };
        let (tool, answer) = match task {
            AgentTask::Identify => ("next_cards_to_identify", "identify_cards"),
            AgentTask::Price => ("next_cards_to_price", "price_cards"),
            AgentTask::Crop => ("next_cards_to_crop", "crop_cards"),
        };
        let session = match a.session_id.as_deref().map(str::trim) {
            Some(given) if !given.is_empty() => {
                let id = self.session_id(given)?;
                let s = self.state.db.session(&id)?;
                if !offers(&s.options) {
                    return Err(Error::msg(match task {
                        AgentTask::Identify => format!(
                            "session \"{}\" isn't identified by an AI agent (create one with engine \"agent\")",
                            s.name
                        ),
                        AgentTask::Price => format!(
                            "session \"{}\" isn't priced by an AI agent (create one with prices \"agent\" or \
                             \"ebay_agent\")",
                            s.name
                        ),
                        AgentTask::Crop => format!(
                            "session \"{}\" isn't cropped by an AI agent (create one with crop_engine \"agent\")",
                            s.name
                        ),
                    }));
                }
                Some(id)
            }
            _ => None,
        };
        let timeout = Duration::from_secs(a.wait_seconds.unwrap_or(WAIT_DEFAULT).min(WAIT_MAX));
        let started = Instant::now();
        let runner = &self.state.runner;
        let waiting = loop {
            let waiting = runner.agent_waiting(session.as_deref(), task);
            if !waiting.is_empty() {
                break waiting;
            }
            // Nothing here, but cards wait for another task of the agent's (identified ones for their price):
            // waiting on would wait for cards that only come once those are answered.
            if let Some(there) = self.elsewhere(session.as_deref(), task) {
                return out(json!({"cards": [], "finished": false,
                    "next": format!("no card waits here now, but {there}; then call {tool} again")}));
            }
            let running = match &session {
                Some(id) => runner.is_running(id),
                None => self
                    .state
                    .db
                    .sessions()?
                    .iter()
                    .any(|s| offers(&s.options) && runner.is_running(&s.id)),
            };
            if !running {
                let what = match task {
                    AgentTask::Identify => "identified",
                    AgentTask::Price => "priced",
                    AgentTask::Crop => "cropped",
                };
                return out(json!({"cards": [], "finished": true,
                    "next": format!("no session {what} by an AI agent is running: nothing more will come")}));
            }
            if started.elapsed() >= timeout {
                return out(json!({"cards": [], "finished": false,
                    "next": format!("the session still runs (e.g. scanning): call {tool} again")}));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        };
        let long_edge = a
            .long_edge
            .unwrap_or(PICTURE_LONG_EDGE)
            .clamp(256, imaging::PREVIEW_LONG_EDGE);
        // One card by default: its two pictures fit a result as they are, where three cards' rarely do.
        let max = a.max.unwrap_or(1).clamp(1, 10);
        let mut items = Vec::new();
        let mut shown = Vec::new();
        // The listings Cardloader found for a card whose agent says which of them are the card, with thumbnails.
        let mut found: Option<(EbayMarket, Vec<ListingPicture>)> = None;
        for (session_id, item_id) in waiting.iter().take(max) {
            let item = self.state.db.item(*item_id)?;
            let picks = task == AgentTask::Price
                && self
                    .state
                    .db
                    .session(session_id)?
                    .options
                    .agent_picks_listings();
            // Such a card comes alone: its listings' pictures take the room other cards would.
            if picks && !items.is_empty() {
                break;
            }
            shown.push(pictures(&item, long_edge).await?);
            if let Some(market) = item.market.clone().filter(|_| picks) {
                let thumbs = thumbnails(&market.listings).await;
                found = Some((market, thumbs));
            }
            items.push((session_id, item));
            if picks {
                break;
            }
        }
        // As many of them as one result holds; the others go on waiting and come with the next call.
        let thumbs_size: usize = found
            .iter()
            .flat_map(|(_, thumbs)| thumbs)
            .map(ListingPicture::size)
            .sum();
        fit(&mut shown, long_edge, PICTURES_BUDGET - thumbs_size).await?;
        let mut cards = Vec::new();
        let mut pics = Vec::new();
        for ((session_id, item), shown) in items.iter().zip(&shown) {
            let item_id = item.summary.id;
            let s = self.state.db.session(session_id)?;
            let mut card = json!({
                "itemId": item_id,
                "sessionId": s.id,
                "session": s.name,
                "cardType": s.id_mode,
                "game": s.options.subcategory,
                "files": {"front": item.summary.front_path, "back": item.summary.back_path},
            });
            if task == AgentTask::Price {
                let i = &item.summary;
                let c = &i.card;
                card["cardType"] = json!(i.id_type);
                card["game"] = json!(c.subcategory);
                card["card"] = json!({
                    "name": c.name, "fullName": c.full_name, "setName": c.set_name, "setCode": c.set_code,
                    "cardNumber": c.card_number, "year": c.year, "variant": c.series, "brand": c.brand,
                    "serial": c.serial, "features": c.features,
                });
                card["slab"] = json!(i.slab);
                card["ebayRange"] = json!(i.market_range);
                // The site its listings are searched on (find_ebay_listings' marketplace), prices in its currency.
                card["ebaySite"] = json!(Some(s.options.ebay_site.clone())
                    .filter(|site| !site.is_empty())
                    .unwrap_or_else(|| self.state.settings.get().ebay.search_marketplace));
            }
            let (shown, sizes) = blocks(shown);
            if task == AgentTask::Crop {
                // The frame the corners are given in: these pictures' own pixels.
                card["pictures"] = sizes;
            }
            pics.extend(shown);
            if let Some((market, thumbs)) = &found {
                card["ebayQuery"] = json!(market.query);
                card["listings"] = json!(market
                    .listings
                    .iter()
                    .map(|l| {
                        let pictured = thumbs.iter().any(|t| t.id == l.id && t.picture.is_ok());
                        json!({"id": l.id, "title": l.title, "price": l.price, "currency": l.currency,
                               "condition": l.condition, "picture": pictured})
                    })
                    .collect::<Vec<_>>());
                pics.extend(listing_blocks(thumbs));
            }
            cards.push(card);
        }
        // Whether the agent may look beyond the pictures is the user's choice (Settings → AI agents), said with
        // every batch: Cardloader can't switch off an agent's own web search.
        let web = self.state.settings.get().mcp.allow_web;
        let next = match (task, web) {
            (AgentTask::Price, _) if found.is_some() => format!(
                "say which of the card's listings are this card, as it is: the same card and number, the same \
                 variant or parallel, and the same state (a raw card: ungraded listings only; a slab: the same \
                 grader and grade); no lots, reprints or other cards. Read the titles, and hold the listings' \
                 pictures below against the card's own; listing_pictures gives larger ones of the listings you \
                 aren't sure of. {} Then send their ids with {answer} as listing_ids, with a note on what you \
                 left out: the card's price is their median. An empty list when none is the card. These \
                 listings were searched for the card as it was read: if it was misread, first correct it with \
                 edit_card, then find_ebay_listings searches for the corrected card, and send ids from that search",
                if web {
                    "You may search the web too (the user allows it)."
                } else {
                    "The user turned web search off in Cardloader, so don't search the web."
                }
            ),
            (AgentTask::Identify, false) => format!(
                "read each card from its pictures below only: the user turned web search off in Cardloader, so don't \
                 search the web or other sources. Then send them with {answer}"
            ),
            (AgentTask::Identify, true) => format!(
                "read each card's pictures below; when one is unclear you may search the web (the user allows it) to \
                 confirm its set, number or parallel. Then send them with {answer}"
            ),
            (AgentTask::Price, false) => format!(
                "price each card as it is (raw, or the slab's grade) from Cardloader's tools (find_ebay_listings, \
                 keep_ebay_listings) and what you know: the user turned web search off in Cardloader, so don't \
                 search the web. Then send the prices with {answer}"
            ),
            (AgentTask::Price, true) => format!(
                "price each card as it is (raw, or the slab's grade); you may search the web for recent sales (the \
                 user allows it), and find_ebay_listings helps too. Then send the prices with {answer}"
            ),
            (AgentTask::Crop, _) => format!(
                "find the card in each picture below: its four corners as [x, y] pixels of that picture (its size \
                 is given with it), clockwise from the card's own top-left corner, on the card's edge and not \
                 inside it. preview_crop shows what corners cut out, before anything is saved. Then send them with \
                 {answer}: front, and back for a card that has a back picture"
            ),
        };
        let mut content = out(json!({
            "cards": cards,
            "waiting": waiting.len(),
            "webSearch": web,
            "next": next,
        }))?;
        content.extend(pics);
        Ok(content)
    }

    fn price_cards(&self, a: PriceCards) -> Result<Vec<ContentBlock>> {
        let mut results = Vec::new();
        for card in a.cards {
            let item_id = card.item_id;
            let note = card.note.unwrap_or_default().trim().to_string();
            // Checked here, before the card's wait is answered: an answer handed over ends the wait.
            let answer = match (card.unpriced, card.listing_ids, card.price) {
                (Some(true), ..) => Ok(AgentAnswer::Unpriced {
                    note: if note.is_empty() {
                        "no price found".into()
                    } else {
                        note
                    },
                }),
                (_, Some(ids), _) => self
                    .known_listings(item_id, &ids)
                    .map(|()| AgentAnswer::Kept { ids, note }),
                (_, None, Some(_)) if self.picks_listings(item_id) => Err(Error::msg(
                    "this session prices a card from its eBay listings that you keep: send listing_ids (the \
                     listings that are this card, [] when none is), or unpriced with a note",
                )),
                (_, None, Some(price)) if price.is_finite() && price >= 0.0 => {
                    currency_code(card.currency.as_deref()).map(|currency| AgentAnswer::Priced {
                        price: (price * 100.0).round() / 100.0,
                        currency,
                        note,
                    })
                }
                (_, None, Some(_)) => Err(Error::msg("the price must be a number, 0 or more")),
                (_, None, None) => Err(Error::msg(
                    "give the card's price or its listing_ids, or unpriced with a note",
                )),
            };
            let result = answer.and_then(|answer| self.state.runner.answer_agent(item_id, answer));
            results.push(match result {
                Ok(()) => json!({"itemId": item_id, "ok": true}),
                Err(e) => json!({"itemId": item_id, "ok": false, "error": e.to_string()}),
            });
        }
        out(json!({
            "results": results,
            "stillWaiting": self.state.runner.agent_waiting(None, AgentTask::Price).len(),
            "next": self.after(AgentTask::Price, "next_cards_to_price"),
        }))
    }

    /// The agent's other tasks that have cards waiting now (of `session`, or of any): a session can have it
    /// identify, price and crop, and a card goes from one to the next. Asked only to identify, an agent stopped
    /// there with the cards waiting for its prices.
    fn elsewhere(&self, session: Option<&str>, task: AgentTask) -> Option<String> {
        let tasks = [
            (
                AgentTask::Identify,
                "next_cards_to_identify",
                "to be identified",
            ),
            (AgentTask::Price, "next_cards_to_price", "for your price"),
            (AgentTask::Crop, "next_cards_to_crop", "to be cropped"),
        ];
        let there: Vec<String> = tasks
            .into_iter()
            .filter(|(other, ..)| *other != task)
            .filter_map(|(other, tool, what)| {
                let n = self.state.runner.agent_waiting(session, other).len();
                (n > 0).then(|| format!("{n} card(s) wait {what}: call {tool}"))
            })
            .collect();
        (!there.is_empty()).then(|| there.join(", and "))
    }

    /// What an agent does after answering cards of `task`: its loop again, by way of the other tasks' cards.
    fn after(&self, task: AgentTask, tool: &str) -> String {
        match self.elsewhere(None, task) {
            Some(there) => format!("{there}; then call {tool} again, until it says finished"),
            None => format!(
                "call {tool} again until it says finished: it also says when cards wait for another task of \
                 yours (their price, their corners)"
            ),
        }
    }

    /// Whether the card's session has its agent say which eBay listings are the card (`ebay_agent`).
    fn picks_listings(&self, item_id: i64) -> bool {
        let session = self
            .state
            .db
            .item_summary(item_id)
            .and_then(|i| self.state.db.session(&i.session_id));
        session.is_ok_and(|s| s.options.agent_picks_listings())
    }

    /// The card's last eBay search has every one of these listings.
    fn known_listings(&self, item_id: i64, ids: &[String]) -> Result<()> {
        let market = self.state.db.item(item_id)?.market.ok_or_else(|| {
            Error::msg("the card has no eBay listings yet: find_ebay_listings first")
        })?;
        let unknown: Vec<&str> = ids
            .iter()
            .filter(|id| market.listings.iter().all(|l| l.id != **id))
            .map(String::as_str)
            .collect();
        match unknown.is_empty() {
            true => Ok(()),
            false => Err(Error::msg(format!(
                "no such listing in the card's last search: {}",
                unknown.join(", ")
            ))),
        }
    }

    /// Larger pictures of some of a card's eBay listings, within one result.
    async fn listing_pictures(&self, a: ListingPictures) -> Result<Vec<ContentBlock>> {
        self.known_listings(a.item_id, &a.listing_ids)?;
        let market = self.state.db.item(a.item_id)?.market.unwrap_or_default();
        let edge = a
            .long_edge
            .unwrap_or(LISTING_PICTURE_EDGE)
            .clamp(256, PICTURE_LONG_EDGE);
        let asked: Vec<&MarketListing> = a
            .listing_ids
            .iter()
            .filter_map(|id| market.listings.iter().find(|l| l.id == *id))
            .collect();
        let (now, later) = asked.split_at(asked.len().min(LISTING_PICTURES));
        let each = PICTURES_BUDGET / now.len().max(1);
        let shown = listing_pictures(now, edge, Some(each)).await;
        let mut content = out(json!({
            "itemId": a.item_id,
            "pictures": shown.iter().map(|p| match &p.picture {
                Ok((_, (w, h))) => json!({"id": p.id, "title": p.title, "width": w, "height": h}),
                Err(e) => json!({"id": p.id, "title": p.title, "error": e}),
            }).collect::<Vec<_>>(),
            "notShown": later.iter().map(|l| l.id.clone()).collect::<Vec<_>>(),
            "next": match later.is_empty() {
                true => "compare them with the card's pictures (get_card shows those again)",
                false => "call listing_pictures again for the listings in notShown",
            },
        }))?;
        content.extend(listing_blocks(&shown));
        Ok(content)
    }

    /// One side's corners as an agent gives them, read as fractions of the picture it was shown.
    fn quad(item: &ItemDetail, side: &str, size: [f64; 2], corners: [[f64; 2]; 4]) -> Result<Quad> {
        let path = shown(item, side)
            .ok_or_else(|| Error::msg(format!("card {} has no {side} picture", item.summary.id)))?;
        // Previews are plain JPEGs, their header says their size. A scan shown for want of a preview may be
        // turned by its EXIF data, so its shape isn't checked.
        let previewed = match side {
            "back" => item.back_preview_path.is_some(),
            _ => item.preview_path.is_some(),
        };
        if let Some((w, h)) = image::image_dimensions(&path).ok().filter(|_| previewed) {
            let shape = (size[0] / size[1]) / (f64::from(w) / f64::from(h));
            if !shape.is_finite() || (shape - 1.0).abs() > SHAPE_TOLERANCE {
                return Err(Error::msg(format!(
                    "{side}: size {}×{} isn't the shape of the picture: give the width and height of the \
                     picture the corners are in, as given with it",
                    size[0], size[1]
                )));
            }
        }
        Quad::from_pixels((size[0], size[1]), corners)
            .map_err(|why| Error::msg(format!("{side}: {why}")))
    }

    /// What an agent's answer about one card's corners means for the card.
    fn crop_answer(&self, card: CardCorners) -> Result<AgentAnswer> {
        let note = card.note.unwrap_or_default().trim().to_string();
        if card.uncropped == Some(true) {
            return Ok(AgentAnswer::Uncropped {
                note: if note.is_empty() {
                    "no card found in the picture".into()
                } else {
                    note
                },
            });
        }
        let item = self.state.db.item(card.item_id)?;
        let front = card
            .front
            .ok_or_else(|| Error::msg("give the front's corners, or uncropped with a note"))?;
        let mut sides: Vec<SideCorners> = vec![(
            "front",
            Ok(Self::quad(&item, "front", front.size, front.corners)?),
        )];
        // A back left out can't be told from one forgotten, so a card with a back gets an answer about it.
        if shown(&item, "back").is_some() {
            let why = card.no_back.map(|w| w.trim().to_string());
            sides.push((
                "back",
                match (card.back, why) {
                    (Some(back), _) => Ok(Self::quad(&item, "back", back.size, back.corners)?),
                    (None, Some(why)) if !why.is_empty() => Err(why),
                    _ => {
                        return Err(Error::msg(
                            "this card has a back picture: give back's corners too, or no_back with why not",
                        ))
                    }
                },
            ));
        }
        Ok(AgentAnswer::Cropped { sides, note })
    }

    fn crop_cards(&self, a: CropCards) -> Result<Vec<ContentBlock>> {
        let mut results = Vec::new();
        for card in a.cards {
            let item_id = card.item_id;
            // Checked here, where the agent can correct it: once handed over, the card no longer waits.
            let result = self
                .crop_answer(card)
                .and_then(|answer| self.state.runner.answer_agent(item_id, answer));
            results.push(match result {
                Ok(()) => json!({"itemId": item_id, "ok": true}),
                Err(e) => json!({"itemId": item_id, "ok": false, "error": e.to_string()}),
            });
        }
        out(json!({
            "results": results,
            "stillWaiting": self.state.runner.agent_waiting(None, AgentTask::Crop).len(),
            "next": self.after(AgentTask::Crop, "next_cards_to_crop"),
        }))
    }

    /// The cut that corners give, from the picture the agent was shown: nothing is saved.
    async fn preview_crop(&self, a: PreviewCrop) -> Result<Vec<ContentBlock>> {
        let item = self.state.db.item(a.item_id)?;
        let side = match a.side.as_deref().map(str::trim) {
            None | Some("") | Some("front") => "front",
            Some("back") => "back",
            Some(other) => return Err(Error::msg(format!("side is front or back, not {other}"))),
        };
        let quad = Self::quad(&item, side, a.size, a.corners)?;
        let path = shown(&item, side)
            .ok_or_else(|| Error::msg(format!("card {} has no {side} picture", a.item_id)))?;
        let (data, (w, h)) = tokio::task::spawn_blocking(move || -> Result<_> {
            let cut = crate::crop::cut(&imaging::open(Path::new(&path))?, &quad);
            let cut = imaging::fit_long_edge(&cut, CUT_PREVIEW_LONG_EDGE);
            Ok((
                imaging::b64(&imaging::jpeg_bytes(&cut, 85)?),
                (cut.width(), cut.height()),
            ))
        })
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
        let mut content = out(json!({
            "itemId": a.item_id,
            "side": side,
            "width": w,
            "height": h,
            "saved": false,
            "next": "the cut these corners give is below: when the whole card is in it, upright, with no more than \
                     a sliver of background, send them with crop_cards; else correct the corners and look again",
        }))?;
        content.push(ContentBlock::image(data, "image/jpeg"));
        Ok(content)
    }

    async fn find_ebay_listings(&self, a: FindEbayListings) -> Result<Vec<ContentBlock>> {
        let market = self
            .state
            .find_ebay_listings(
                a.item_id,
                a.query,
                a.include_auctions.unwrap_or(false),
                a.limit,
                a.marketplace,
            )
            .await?;
        // Titles and prices are what tells the card's listings from others: no pictures (tokens).
        let listings: Vec<Value> = market
            .listings
            .iter()
            .map(|l| {
                json!({"id": l.id, "title": l.title, "price": l.price, "currency": l.currency,
                       "condition": l.condition, "buying": l.buying})
            })
            .collect();
        out(json!({
            "itemId": a.item_id,
            "query": market.query,
            "marketplace": market.marketplace,
            "totalOnEbay": market.total,
            "range": market.range,
            "listings": listings,
            "next": "keep_ebay_listings with the ids of the listings that are this card (same card, number, \
                     parallel and grade; no lots): the range then counts them alone",
        }))
    }

    fn keep_ebay_listings(&self, a: KeepEbayListings) -> Result<Vec<ContentBlock>> {
        let market = self.state.keep_ebay_listings(
            a.item_id,
            &a.listing_ids,
            a.note.as_deref().unwrap_or_default(),
        )?;
        out(json!({
            "itemId": a.item_id,
            "kept": market.listings.iter().filter(|l| l.counted).count(),
            "leftOut": market.listings.iter().filter(|l| !l.counted).count(),
            "range": market.range,
        }))
    }

    fn identify_cards(&self, a: IdentifyCards) -> Result<Vec<ContentBlock>> {
        let mut results = Vec::new();
        for card in a.cards {
            let item_id = card.item_id;
            let result = self
                .agent_answer(card)
                .and_then(|answer| self.state.runner.answer_agent(item_id, answer));
            results.push(match result {
                Ok(()) => json!({"itemId": item_id, "ok": true}),
                Err(e) => json!({"itemId": item_id, "ok": false, "error": e.to_string()}),
            });
        }
        out(json!({
            "results": results,
            "stillWaiting": self.state.runner.agent_waiting(None, AgentTask::Identify).len(),
            "next": self.after(AgentTask::Identify, "next_cards_to_identify"),
        }))
    }

    /// Checks an agent's identification of a card and turns it into what the session stores.
    fn agent_answer(&self, c: CardIdentity) -> Result<AgentAnswer> {
        let text = |v: Option<String>| v.map(|t| t.trim().to_string()).unwrap_or_default();
        let note = text(c.note);
        if c.unidentified == Some(true) {
            return Ok(AgentAnswer::Unidentified {
                note: if note.is_empty() {
                    "couldn't identify the card".into()
                } else {
                    note
                },
            });
        }
        let name = text(c.name);
        if name.is_empty() {
            return Err(Error::msg(
                "give the card's name, or send it with unidentified and why",
            ));
        }
        let subcategory = match c.game.as_deref().map(str::trim) {
            Some(g) if !g.is_empty() => game(g).unwrap_or_else(|_| g.to_string()),
            _ => String::new(),
        };
        let session_kind = self
            .state
            .db
            .item_summary(c.item_id)
            .and_then(|i| self.state.db.session(&i.session_id))
            .map(|s| s.id_mode)?;
        let id_type = match c
            .card_type
            .as_deref()
            .map(|t| t.trim().to_ascii_lowercase())
        {
            Some(t) if ["tcg", "sport", "comics"].contains(&t.as_str()) => t,
            Some(t) if !t.is_empty() => {
                return Err(Error::msg(format!(
                    "card_type is tcg, sport or comics, not \"{t}\""
                )))
            }
            _ if SPORTS.contains(&subcategory.as_str()) => "sport".into(),
            _ if TCG_GAMES.contains(&subcategory.as_str()) => "tcg".into(),
            _ => match session_kind {
                IdMode::Sport => "sport".into(),
                IdMode::Comics => "comics".into(),
                _ => "tcg".into(),
            },
        };
        let certainty = match c
            .certainty
            .as_deref()
            .map(|t| t.trim().to_ascii_lowercase())
        {
            Some(t) if ["high", "medium", "low"].contains(&t.as_str()) => Some(t),
            Some(t) if !t.is_empty() => return Err(Error::msg("certainty is high, medium or low")),
            _ => None,
        };
        let mut card = CardFields {
            name,
            full_name: text(c.full_name),
            set_name: text(c.set_name),
            set_code: text(c.set_code),
            card_number: text(c.card_number),
            series: text(c.variant),
            year: text(c.year),
            subcategory,
            brand: text(c.brand),
            team: text(c.team),
            serial: text(c.serial),
            features: text(c.features),
        };
        if card.full_name.is_empty() {
            card.full_name = card.composed_full_name();
        }
        Ok(AgentAnswer::Identified {
            id_type,
            card: Box::new(card),
            certainty,
            note,
        })
    }

    fn search_cards(&self, a: SearchCards) -> Result<Vec<ContentBlock>> {
        let found =
            self.state
                .db
                .search_cards(&a.query, &a.filter, a.limit.unwrap_or(50).clamp(1, 200))?;
        let cards: Vec<Value> = found
            .cards
            .iter()
            .map(|c| {
                let mut v = card_row(&c.item);
                v["sessionId"] = json!(c.item.session_id);
                v["sessionName"] = json!(c.session_name);
                v["copies"] = json!(c.copies);
                v
            })
            .collect();
        out(
            json!({"matching": found.total, "value": found.value, "values": found.values, "cards": cards}),
        )
    }

    async fn inspect(&self, a: PathArg) -> Result<Vec<ContentBlock>> {
        let path = expand(&a.path);
        let sessions: Vec<Value> = self
            .state
            .db
            .sessions()?
            .iter()
            .filter(|s| Path::new(&s.source_ref) == path)
            .map(|s| json!({"id": s.id, "name": s.name, "status": s.status, "cards": s.counts.total}))
            .collect();
        if path.is_file() {
            let file = path.clone();
            let preview = tokio::task::spawn_blocking(move || csv_file::preview(&file))
                .await
                .map_err(|e| Error::msg(e.to_string()))??;
            return out(json!({"path": path, "csv": preview, "sessionsUsingIt": sessions}));
        }
        let images = source::list_images(&path)?;
        let names: Vec<String> = images
            .iter()
            .take(12)
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        out(json!({
            "path": path,
            "images": images.len(),
            "firstFiles": names,
            "sessionsUsingIt": sessions,
        }))
    }

    async fn create_session(&self, a: CreateSession) -> Result<Vec<ContentBlock>> {
        let settings = self.state.settings.get();
        let path = expand(&a.path);
        let source = a.source.unwrap_or(SourceKind::Folder);
        if source == SourceKind::Scanner {
            return Err(Error::msg("scanner sessions are started in the app"));
        }
        let id_mode = a.card_type.unwrap_or(settings.default_id_mode);
        // As the New session dialog does: a local session of sports cards reads both sides unless told not to.
        let engine = a.engine.unwrap_or(settings.engine);
        let local_sport = engine == Engine::Local && id_mode == IdMode::Sport;
        let read_backs = a.read_backs.unwrap_or(local_sport);
        let task = a.task.unwrap_or(settings.default_task);
        let save_crops = task != Task::Crop && a.save_crops.unwrap_or(settings.default_save_crops);
        let crop_folder = match a.crop_folder.as_deref().map(str::trim) {
            Some(folder) if !folder.is_empty() && (task == Task::Crop || save_crops) => {
                self.crop_folder(folder)?
            }
            _ => String::new(),
        };
        let price_stats = a.price_stats.unwrap_or(settings.price_stats);
        let options = SessionOptions {
            task,
            save_crops,
            crop_folder,
            crop_format: a.crop_format.unwrap_or(settings.default_crop_format),
            price_stats,
            prices: Some(a.prices.unwrap_or(if price_stats {
                Prices::Ximilar
            } else {
                Prices::Off
            })),
            grade_engine: a.grade_engine,
            crop_engine: a.crop_engine,
            ebay_site: a.ebay_site.clone().unwrap_or_default(),
            grading: a.grading.unwrap_or(settings.default_grading),
            condition_mode: settings.default_condition_mode.clone(),
            pairing: a.pairing.unwrap_or(if local_sport && read_backs {
                Pairing::FrontBack
            } else {
                settings.default_pairing
            }),
            both_sides: read_backs,
            skip_detection: a.cropped.unwrap_or(false),
            known_category: a.card_type.is_some_and(|t| t != IdMode::Auto),
            known_side: a.known_side.unwrap_or(false),
            known_rotation: a.upright.unwrap_or(false),
            alphabet: match a.alphabet.as_deref() {
                Some(text) => alphabet(text)?,
                None => String::new(),
            },
            subcategory: match a.game.as_deref() {
                Some(text) => game(text)?,
                None => settings.default_subcategory.clone(),
            },
            scan: settings.scan.clone(),
            sync_remote: a.store_in_cards_cloud.unwrap_or(settings.sync_by_default),
            engine,
            local_model: settings.local_model.clone(),
            ..SessionOptions::default()
        };
        let name = match a.name.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => default_name(&path),
        };
        let new = NewSession {
            name,
            source_kind: source,
            source_ref: path.to_string_lossy().into_owned(),
            id_mode,
            options,
        };
        let session = if source == SourceKind::Csv {
            self.state
                .import_csv(new, a.reanalyse.unwrap_or(false))
                .await?
        } else {
            source::list_images(&path)?;
            self.state
                .create_session(new, a.continue_cards_cloud_session)
                .await?
        };
        let start = a.start.unwrap_or(true) && session.status != SessionStatus::Completed;
        if start {
            self.state.runner.start(&session.id)?;
        }
        let session = self.state.db.session(&session.id)?;
        let mut v = self.session_json(&session, true);
        if session.options.crops() {
            v["cropsFolder"] = json!(self.state.runner.crop_dir(&session));
        }
        if start {
            v["next"] = json!(next_for(&session.options));
        }
        out(v)
    }

    async fn wait(&self, a: WaitForSession) -> Result<Vec<ContentBlock>> {
        let id = self.session_id(&a.session_id)?;
        let timeout =
            Duration::from_secs(a.timeout_seconds.unwrap_or(WAIT_DEFAULT).clamp(1, WAIT_MAX));
        let started = Instant::now();
        while self.state.runner.is_running(&id) && started.elapsed() < timeout {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        let mut v = self.session_json(&self.state.db.session(&id)?, false);
        v["finished"] = json!(!self.state.runner.is_running(&id));
        out(v)
    }

    fn accept(&self, a: AcceptCards) -> Result<Vec<ContentBlock>> {
        let accepted = a.accepted.unwrap_or(true);
        let by = agent(a.note);
        let mut changed = 0;
        let mut skipped = Vec::new();
        for (session, ids) in self.by_session(&a.item_ids)? {
            let mut identified = Vec::new();
            for id in ids {
                let item = self.state.db.item_summary(id)?;
                if !matches!(item.status, ItemStatus::Done | ItemStatus::Grading) {
                    skipped.push(json!({"id": id, "why": format!("{} cards can't be accepted", item.status.as_str())}));
                } else if item.reviewed && !item.agent_reviewed {
                    skipped.push(json!({"id": id, "why": "the user accepted it; it stays theirs"}));
                } else {
                    identified.push(id);
                }
            }
            changed += self
                .state
                .runner
                .review_items(&session, &identified, accepted, &by)?;
        }
        out(json!({"changed": changed, "skipped": skipped}))
    }

    async fn edit(&self, a: EditCard) -> Result<Vec<ContentBlock>> {
        let current = self.state.db.item_summary(a.item_id)?;
        let mut card = current.card.clone();
        let fields = [
            (&mut card.name, a.name),
            (&mut card.full_name, a.full_name),
            (&mut card.set_name, a.set_name),
            (&mut card.set_code, a.set_code),
            (&mut card.card_number, a.card_number),
            (&mut card.series, a.series),
            (&mut card.year, a.year),
            (&mut card.subcategory, a.subcategory),
            (&mut card.brand, a.brand),
            (&mut card.team, a.team),
            (&mut card.serial, a.serial),
            (&mut card.features, a.features),
        ];
        for (field, value) in fields {
            if let Some(value) = value {
                *field = value;
            }
        }
        let edit = ItemEdit {
            card,
            price: a.price.or(current.price),
            notes: a.notes.unwrap_or(current.notes),
            location: a.location.unwrap_or(current.location),
            title: a.title.unwrap_or(current.title),
        };
        self.state
            .runner
            .edit_item(a.item_id, edit, &agent(a.note))?;
        self.card(a.item_id, false).await
    }

    fn edit_many(&self, a: EditCards) -> Result<Vec<ContentBlock>> {
        let field = match a.field.trim().to_ascii_lowercase().as_str() {
            "title" => EditField::Title,
            "features" => EditField::Features,
            "brand" => EditField::Brand,
            "team" => EditField::Team,
            "series" | "variant" => EditField::Series,
            _ => {
                return Err(Error::msg(
                    "field is title, features, brand, team or series",
                ))
            }
        };
        let place = match a.place.as_deref().map(|p| p.trim().to_ascii_lowercase()) {
            None => EditPlace::End,
            Some(p) => match p.as_str() {
                "" | "end" => EditPlace::End,
                "start" => EditPlace::Start,
                "replace" => EditPlace::Replace,
                _ => return Err(Error::msg("place is start, end or replace")),
            },
        };
        let edit = BulkEdit {
            field,
            place,
            text: a.text,
        };
        // What the change would give is read before it is made: afterwards nothing would be left to change.
        let titles = self.state.preview_titles(TitleQuery {
            item_ids: Some(a.item_ids.clone()),
            edit: Some(edit.clone()),
            templates: None,
        })?;
        let done = match a.preview.unwrap_or(false) {
            true => EditedItems::default(),
            false => self.state.edit_items(&a.item_ids, &edit, &agent(None))?,
        };
        let titles: Vec<Value> = titles
            .iter()
            .map(|t| {
                json!({"id": t.item_id, "seq": t.seq, "title": t.title, "length": t.title.chars().count(),
                    "custom": t.custom, "leftOut": t.left_out, "changes": t.changed, "problem": t.problem})
            })
            .collect();
        out(json!({"changed": done.changed, "skipped": done.skipped, "titles": titles}))
    }

    fn update_defaults(&self, a: UpdateDefaults) -> Result<Vec<ContentBlock>> {
        let mut s = self.state.settings.get();
        if let Some(v) = a.task {
            s.default_task = v;
        }
        if let Some(v) = a.save_crops {
            s.default_save_crops = v;
        }
        if let Some(v) = a.card_type {
            s.default_id_mode = v;
        }
        if let Some(v) = a.game {
            s.default_subcategory = if v.trim().is_empty() {
                String::new()
            } else {
                game(&v)?
            };
        }
        if let Some(v) = a.grading {
            s.default_grading = v;
        }
        if let Some(v) = a.pairing {
            s.default_pairing = v;
        }
        if let Some(v) = a.condition_scale {
            let scales = ["tcgplayer", "cardmarket", "ebay_tcg", "ebay_sport"];
            if !scales.contains(&v.as_str()) {
                return Err(Error::msg(format!(
                    "the condition scale is one of {}",
                    scales.join(", ")
                )));
            }
            s.default_condition_mode = v;
        }
        if let Some(v) = a.store_in_cards_cloud {
            s.sync_by_default = v;
        }
        if let Some(v) = a.price_stats {
            s.price_stats = v;
        }
        if let Some(v) = a.concurrency {
            s.concurrency = v;
        }
        if let Some(v) = a.folder {
            s.default_folder =
                Some(expand(&v).to_string_lossy().into_owned()).filter(|f| !f.is_empty());
        }
        self.state.settings.update(s)?;
        out(json!({"defaults": defaults(&self.state.settings.get())}))
    }

    /// The cards' ids grouped by their session, which the runner's batch calls take.
    fn by_session(&self, ids: &[i64]) -> Result<BTreeMap<String, Vec<i64>>> {
        let mut groups: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        for &id in ids {
            let item = self
                .state
                .db
                .item_summary(id)
                .map_err(|_| Error::msg(format!("no card has id {id}")))?;
            groups.entry(item.session_id).or_default().push(id);
        }
        Ok(groups)
    }

    fn identified(&self, session_id: &str) -> Result<impl Iterator<Item = ItemSummary>> {
        Ok(self
            .state
            .db
            .items(session_id)?
            .into_iter()
            .filter(|i| matches!(i.status, ItemStatus::Done | ItemStatus::Grading)))
    }

    /// The cards to export: as given, else every identified card at its market price (0 without one).
    fn priced(&self, session_id: &str, items: Option<Vec<PricedItem>>) -> Result<Vec<PricedItem>> {
        match items {
            Some(items) => Ok(items),
            None => Ok(self
                .identified(session_id)?
                .map(|i| PricedItem {
                    item_id: i.id,
                    price: i.usd_price().unwrap_or(0.0),
                })
                .collect()),
        }
    }

    /// A crop folder an agent names: absolute, and outside the app's folders (as `export_path`).
    fn crop_folder(&self, folder: &str) -> Result<String> {
        let path = expand(folder);
        if !path.is_absolute() {
            return Err(Error::msg("give the crop folder's absolute path"));
        }
        // Checked before anything is made: the deepest folder that exists, with links resolved, and the rest of the path.
        let existing = path
            .ancestors()
            .find(|a| a.exists())
            .ok_or_else(|| Error::msg("no such folder"))?;
        let rest = path.strip_prefix(existing).unwrap_or(Path::new(""));
        if rest
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(Error::msg("give the crop folder's path without .."));
        }
        let real = existing.canonicalize()?.join(rest);
        let data = self.state.runner.data_dir.as_path();
        let config = self.state.settings.path().parent();
        if std::iter::once(data)
            .chain(config)
            .filter_map(|d| d.canonicalize().ok())
            .any(|d| real.starts_with(d))
        {
            return Err(Error::msg(
                "crops can't be written into Cardloader's own folders; choose another folder",
            ));
        }
        std::fs::create_dir_all(&real)?;
        Ok(real.to_string_lossy().into_owned())
    }

    /// Where an export may write (see `writable`): never in the app's data folder (the database, sessions'
    /// files) or its config folder (settings).
    fn export_path(&self, path: &str, overwrite: Option<bool>) -> Result<PathBuf> {
        let data = self.state.runner.data_dir.as_path();
        let config = self.state.settings.path().parent();
        let app_dirs: Vec<&Path> = std::iter::once(data).chain(config).collect();
        writable(path, overwrite, &app_dirs)
    }
}

/// What the agent does after a session starts or gets tasks added: the loops of the tasks that are its own.
fn next_for(o: &SessionOptions) -> String {
    let loops: Vec<&str> = [
        (
            o.agent(),
            "next_cards_to_identify and identify the cards with identify_cards",
        ),
        (
            o.prices_by_agent(),
            "next_cards_to_price and price the cards with price_cards",
        ),
        (
            o.crops_by_agent(),
            "next_cards_to_crop and give the cards' corners with crop_cards",
        ),
    ]
    .into_iter()
    .filter_map(|(on, what)| on.then_some(what))
    .collect();
    match loops.as_slice() {
        [] => "call wait_for_session until it finishes".into(),
        [one] => format!("call {one} until it says finished"),
        several => format!(
            "call {}, each until it says finished",
            several.join(", then ")
        ),
    }
}

/// The picture of a card's side ("front", "back") an agent is shown: its preview, else the scan.
fn shown(item: &ItemDetail, side: &str) -> Option<String> {
    let s = &item.summary;
    match side {
        "back" => item.back_preview_path.clone().or(s.back_path.clone()),
        _ => item.preview_path.clone().or(Some(s.front_path.clone())),
    }
    .filter(|p| !p.is_empty())
}

/// One side of a card as an agent is shown it.
struct Shown {
    item_id: i64,
    /// "front" or "back".
    side: &'static str,
    path: String,
    /// The JPEG as base64 with its width and height, or why there is none.
    picture: std::result::Result<(String, (u32, u32)), String>,
}

impl Shown {
    /// What it takes of a result: its base64 characters.
    fn size(&self) -> usize {
        self.picture.as_ref().map_or(0, |(data, _)| data.len())
    }
}

/// A card's front and back pictures for an agent, at `long_edge` and the quality they are best read at.
async fn pictures(item: &ItemDetail, long_edge: u32) -> Result<Vec<Shown>> {
    let item_id = item.summary.id;
    let mut sides = Vec::new();
    for side in ["front", "back"] {
        let Some(path) = shown(item, side) else {
            continue;
        };
        let from = path.clone();
        let picture = tokio::task::spawn_blocking(move || {
            imaging::encode_sized(Path::new(&from), long_edge, 85)
        })
        .await
        .map_err(|e| Error::msg(e.to_string()))?;
        sides.push(Shown {
            item_id,
            side,
            path,
            picture: picture.map_err(|e| e.to_string()),
        });
    }
    Ok(sides)
}

/// Makes the cards' pictures fit one tool result (`PICTURES_BUDGET`). When they don't as they are, cards are left
/// for the next call first, as long as each picture would get less than `PICTURE_MIN_SHARE` (the first card
/// always stays), and then the pictures over their share are made again within it: coarser, then smaller.
async fn fit(cards: &mut Vec<Vec<Shown>>, long_edge: u32, budget: usize) -> Result<()> {
    let size = |cards: &[Vec<Shown>]| cards.iter().flatten().map(Shown::size).sum::<usize>();
    let count = |cards: &[Vec<Shown>]| cards.iter().flatten().filter(|s| s.size() > 0).count();
    while size(cards) > budget
        && cards.len() > 1
        && budget / count(cards).max(1) < PICTURE_MIN_SHARE
    {
        cards.pop();
    }
    if size(cards) <= budget {
        return Ok(());
    }
    let share = budget / count(cards).max(1);
    for shown in cards.iter_mut().flatten().filter(|s| s.size() > share) {
        let path = shown.path.clone();
        let picture = tokio::task::spawn_blocking(move || {
            imaging::encode_within(Path::new(&path), long_edge, share)
        })
        .await
        .map_err(|e| Error::msg(e.to_string()))?;
        shown.picture = picture.map_err(|e| e.to_string());
    }
    Ok(())
}

/// A listing's picture as an agent is shown it.
struct ListingPicture {
    id: String,
    title: String,
    /// The JPEG as base64 with its width and height, or why there is none.
    picture: std::result::Result<(String, (u32, u32)), String>,
}

impl ListingPicture {
    fn size(&self) -> usize {
        self.picture.as_ref().map_or(0, |(data, _)| data.len())
    }
}

/// The listings' pictures from eBay at about `edge` pixels, all fetched at once and in the listings' order; each
/// within `each` base64 characters when given.
async fn listing_pictures(
    listings: &[&MarketListing],
    edge: u32,
    each: Option<usize>,
) -> Vec<ListingPicture> {
    let fetched = futures::future::join_all(listings.iter().map(|l| async move {
        let url = l.image.as_deref().ok_or("the listing has no picture")?;
        let bytes = crate::ebay::pictures::fetch(url, edge).await?;
        tokio::task::spawn_blocking(move || match each {
            Some(max) => imaging::encode_bytes_within(&bytes, edge, max),
            None => imaging::encode_bytes(&bytes, edge, 80),
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|_| "the picture couldn't be read".to_string())
    }))
    .await;
    listings
        .iter()
        .zip(fetched)
        .map(|(l, picture)| ListingPicture {
            id: l.id.clone(),
            title: l.title.clone(),
            picture,
        })
        .collect()
}

/// Small pictures of a card's listings to go with it: of the first `LISTING_THUMBS` that have one, as many as
/// `THUMBS_BUDGET` holds.
async fn thumbnails(listings: &[MarketListing]) -> Vec<ListingPicture> {
    let pictured: Vec<&MarketListing> = listings
        .iter()
        .filter(|l| l.image.is_some())
        .take(LISTING_THUMBS)
        .collect();
    let mut size = 0;
    listing_pictures(&pictured, THUMB_EDGE, None)
        .await
        .into_iter()
        .take_while(|p| {
            size += p.size();
            size <= THUMBS_BUDGET
        })
        .collect()
}

/// Listings' pictures as an agent gets them: each after a line with the listing's id and title.
fn listing_blocks(pictures: &[ListingPicture]) -> Vec<ContentBlock> {
    let mut content = Vec::new();
    for p in pictures {
        if let Ok((data, (w, h))) = &p.picture {
            content.push(ContentBlock::text(format!(
                "Listing {} ({w}×{h} px): {}",
                p.id, p.title
            )));
            content.push(ContentBlock::image(data.clone(), "image/jpeg"));
        }
    }
    content
}

/// A card's pictures as an agent gets them: each after a line naming it and its size, and those sizes by side
/// (`{"front": {"width", "height"}, …}`), the frame of coordinates found on them.
fn blocks(shown: &[Shown]) -> (Vec<ContentBlock>, Value) {
    let mut content = Vec::new();
    let mut sizes = json!({});
    for s in shown {
        let label = if s.side == "back" { "Back" } else { "Front" };
        match &s.picture {
            Ok((data, (w, h))) => {
                content.push(ContentBlock::text(format!(
                    "{label} of card {} ({w}×{h} px):",
                    s.item_id
                )));
                content.push(ContentBlock::image(data.clone(), "image/jpeg"));
                sizes[s.side] = json!({"width": w, "height": h});
            }
            Err(e) => content.push(ContentBlock::text(format!("{label}: no picture ({e})"))),
        }
    }
    (content, sizes)
}

fn agent(note: Option<String>) -> Reviewer {
    Reviewer::Agent {
        note: note.unwrap_or_default(),
    }
}

fn keeps(filter: CardFilter, i: &ItemSummary) -> bool {
    let identified = matches!(i.status, ItemStatus::Done | ItemStatus::Grading);
    match filter {
        CardFilter::All => true,
        // As the app counts them (`db::TO_REVIEW`): a card only graded or cropped has nothing to review.
        CardFilter::ToReview => identified && !i.reviewed && i.id_type.is_some(),
        CardFilter::Uncertain => identified && i.confidence.as_deref() != Some("high"),
        CardFilter::AcceptedByAi => i.reviewed && i.agent_reviewed,
        CardFilter::Errors => i.status == ItemStatus::Error,
        CardFilter::Queued => matches!(i.status, ItemStatus::Queued | ItemStatus::Recognizing),
    }
}

/// A card as lists show it to agents.
fn card_row(i: &ItemSummary) -> Value {
    let accepted_by = match (i.reviewed, i.agent_reviewed) {
        (false, _) => None,
        (true, true) => Some("agent"),
        (true, false) => Some("user"),
    };
    json!({
        "id": i.id,
        "seq": i.seq,
        "status": i.status,
        "name": i.card.name,
        "fullName": i.card.full_name,
        "set": i.card.set_name,
        "setCode": i.card.set_code,
        "number": i.card.card_number,
        "year": i.card.year,
        "series": i.card.series,
        "game": i.card.subcategory,
        "brand": i.card.brand,
        "team": i.card.team,
        "serial": i.card.serial,
        "features": i.card.features,
        "customTitle": i.title,
        "type": i.id_type,
        "confidence": i.confidence,
        "distance": i.distance,
        "price": i.price,
        "priceCurrency": i.price_currency,
        "grade": i.grade,
        "condition": i.condition,
        "slab": i.slab,
        "accepted": i.reviewed,
        "acceptedBy": accepted_by,
        "reviewNote": i.review_note,
        "picked": i.picked,
        "edited": i.edited,
        "location": i.location,
        "error": i.error,
        "ebayRange": i.market_range,
    })
}

fn defaults(s: &Settings) -> Value {
    json!({
        "task": s.default_task,
        "saveCrops": s.default_save_crops,
        "cardType": s.default_id_mode,
        "game": s.default_subcategory,
        "grading": s.default_grading,
        "pairing": s.default_pairing,
        "conditionScale": s.default_condition_mode,
        "storeInCardsCloud": s.sync_by_default,
        "priceStats": s.price_stats,
        "ebaySearchSite": s.ebay.search_marketplace,
        "concurrency": s.concurrency,
        "engine": s.engine,
        "folder": s.default_folder,
    })
}

fn allowed(mcp: &McpSettings) -> Value {
    json!({
        "read": true,
        "runAndReview": mcp.allow_run,
        "delete": mcp.allow_delete,
        "marketplaces": mcp.allow_market,
        "settings": mcp.allow_settings,
        "webSearch": mcp.allow_web,
    })
}

/// A currency an agent named (ISO 4217, any case); none is US dollars.
fn currency_code(given: Option<&str>) -> Result<String> {
    let code = given
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .unwrap_or(USD)
        .to_ascii_uppercase();
    if code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(code)
    } else {
        Err(Error::msg(format!(
            "\"{code}\" isn't a currency code: give one like USD, EUR or GBP"
        )))
    }
}

/// The first UUID written in `text` (sessions' ids are UUIDs).
pub(super) fn uuid_in(text: &str) -> Option<&str> {
    let is_uuid = |w: &str| {
        w.len() == 36
            && w.char_indices().all(|(i, c)| match i {
                8 | 13 | 18 | 23 => c == '-',
                _ => c.is_ascii_hexdigit(),
            })
    };
    (0..text.len().saturating_sub(35))
        .filter(|&i| text.is_char_boundary(i) && text.is_char_boundary(i + 36))
        .map(|i| &text[i..i + 36])
        .find(|w| is_uuid(w))
}

/// The game or sport as Ximilar's taxonomy spells it, whatever the case.
fn game(text: &str) -> Result<String> {
    let text = text.trim();
    TCG_GAMES
        .iter()
        .chain(SPORTS)
        .find(|g| g.eq_ignore_ascii_case(text))
        .map(|g| g.to_string())
        .ok_or_else(|| {
            Error::msg(format!(
                "unknown game or sport \"{text}\"; one of: {}",
                TCG_GAMES
                    .iter()
                    .chain(SPORTS)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })
}

/// The writing system: one of `ALPHABETS`, or a language written in it.
fn alphabet(text: &str) -> Result<String> {
    let text = text.trim().to_ascii_lowercase();
    if text.is_empty() {
        return Ok(String::new());
    }
    if ALPHABETS.contains(&text.as_str()) {
        return Ok(text);
    }
    const LATIN: &[&str] = &[
        "english",
        "french",
        "german",
        "spanish",
        "italian",
        "portuguese",
        "dutch",
        "polish",
        "czech",
        "slovak",
    ];
    if LATIN.contains(&text.as_str()) {
        return Ok("latin".into());
    }
    Err(Error::msg(format!(
        "the alphabet is one of {}",
        ALPHABETS.join(", ")
    )))
}

/// A leading `~` is the home folder (agents often write paths so).
fn expand(path: &str) -> PathBuf {
    let path = path.trim();
    match (
        path.strip_prefix("~/").or(path.strip_prefix("~\\")),
        dirs::home_dir(),
    ) {
        (Some(rest), Some(home)) => home.join(rest),
        _ if path == "~" => dirs::home_dir().unwrap_or_default(),
        _ => PathBuf::from(path),
    }
}

/// Where an export may write: an absolute `.csv` path (every export is one CSV file) in an existing folder outside
/// `app_dirs`, not a link, and not an existing file unless allowed. The agent names the path, so without this an
/// export could replace the database, the settings or a card's scan.
fn writable(path: &str, overwrite: Option<bool>, app_dirs: &[&Path]) -> Result<PathBuf> {
    let path = expand(path);
    if !path.is_absolute() {
        return Err(Error::msg("give the file's absolute path"));
    }
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"))
    {
        return Err(Error::msg(
            "exports are CSV files: give a path ending in .csv",
        ));
    }
    // The real folder, with links and `..` resolved, is what must lie outside the app's folders.
    let folder = path
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .filter(|p| p.is_dir())
        .ok_or_else(|| Error::msg(format!("no such folder: {}", path.display())))?;
    if app_dirs
        .iter()
        .filter_map(|d| d.canonicalize().ok())
        .any(|d| folder.starts_with(d))
    {
        return Err(Error::msg(
            "exports can't be written into Cardloader's own folders; choose another folder",
        ));
    }
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(Error::msg(format!(
            "{} is a link; give the path of a file",
            path.display()
        ))),
        Ok(_) if !overwrite.unwrap_or(false) => Err(Error::msg(format!(
            "{} exists; pass overwrite true to replace it",
            path.display()
        ))),
        _ => Ok(path),
    }
}

/// The folder's name and today's date, e.g. "pokemon-en · Oct 27".
fn default_name(path: &Path) -> String {
    let base = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Cards".into());
    let date = chrono::Local::now().format("%b %-d");
    let name = format!("{base} · {date}");
    name.chars()
        .take(crate::ximilar::cards_cloud::MAX_SESSION_NAME)
        .collect()
}
