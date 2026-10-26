//! Types shared by the database, the runner and the UI (serialized camelCase for TypeScript).

use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! str_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $text:literal),+ $(,)? } default $default:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $(#[serde(rename = $text)] $variant,)+
        }

        #[allow(dead_code)]
        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text,)+ }
            }

            pub fn parse(text: &str) -> Self {
                match text { $($text => $name::$variant,)+ _ => $name::$default }
            }
        }

        impl Default for $name {
            fn default() -> Self { $name::$default }
        }
    };
}

str_enum!(
    /// Where a session's images come from.
    SourceKind { Folder = "folder", WatchFolder = "watch_folder", Scanner = "scanner", Csv = "csv" } default Folder
);

str_enum!(
    /// Which recognition endpoint a session uses; `Auto` asks `/process` first.
    IdMode { Auto = "auto", Tcg = "tcg", Sport = "sport", Comics = "comics", Slab = "slab" } default Auto
);

str_enum!(
    /// What identifies the cards: the Ximilar API, or a vision language model on this computer.
    Engine { Ximilar = "ximilar", Local = "local" } default Ximilar
);

str_enum!(
    /// Which card-grader endpoint runs after recognition, if any.
    GradingMode { Off = "off", Condition = "condition", Grade = "grade", Centering = "centering" } default Off
);

str_enum!(
    /// How source images become items: one image per card, or front + back pairs.
    Pairing { Single = "single", FrontBack = "front_back" } default Single
);

str_enum!(
    SessionStatus {
        Draft = "draft",
        Running = "running",
        Paused = "paused",
        Stopped = "stopped",
        Completed = "completed",
        Failed = "failed",
    } default Draft
);

str_enum!(
    ItemStatus {
        Queued = "queued",
        Recognizing = "recognizing",
        Grading = "grading",
        Done = "done",
        Error = "error",
    } default Queued
);

str_enum!(
    /// Where a card's eBay listing is: an unpublished offer, live, sold, or ended without a sale.
    EbayStatus { Draft = "draft", Listed = "listed", Sold = "sold", Ended = "ended" } default Draft
);

str_enum!(
    /// An ungraded card's condition, on the four-step scale eBay uses for trading and sports cards (each
    /// marketplace names the steps its own way).
    CardCondition {
        NearMint = "near_mint",
        Excellent = "excellent",
        VeryGood = "very_good",
        Poor = "poor",
    } default NearMint
);

impl CardCondition {
    /// A condition as graders and marketplaces write it ("Near Mint", "Lightly Played", "LP", …), if known.
    pub fn read(text: &str) -> Option<CardCondition> {
        let t = text.trim().to_ascii_lowercase();
        let t = t
            .trim_start_matches("raw - ")
            .trim_end_matches(" or better");
        Some(match t {
            "gem mint" | "mint" | "near mint" | "nm" | "nm-mt" | "near mint-mint" => {
                CardCondition::NearMint
            }
            "excellent" | "ex" | "lightly played" | "light played" | "lp" => {
                CardCondition::Excellent
            }
            "very good" | "vg" | "good" | "moderately played" | "mp" => CardCondition::VeryGood,
            "poor" | "fair" | "heavily played" | "hp" | "damaged" | "dmg" | "played" => {
                CardCondition::Poor
            }
            _ => return None,
        })
    }
}

str_enum!(
    ColorMode { Color = "color", Gray = "gray", Bw = "bw" } default Color
);

str_enum!(
    MultifeedMode { Off = "off", Default = "default" } default Off
);

str_enum!(
    /// How scanned pages are turned before use, for cards fed into the scanner the other way up.
    ScanRotation { None = "none", UpsideDown = "upside_down" } default None
);

/// Scanner settings; tuned for trading cards by default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanSettings {
    pub dpi: u32,
    pub color: ColorMode,
    pub duplex: bool,
    pub page_width_mm: f32,
    pub page_height_mm: f32,
    pub multifeed: MultifeedMode,
    pub rotation: ScanRotation,
    /// Duplex: the scanner's first side of each card is the card's back (cards fed back side up).
    pub back_first: bool,
}

impl Default for ScanSettings {
    fn default() -> Self {
        ScanSettings {
            dpi: 600,
            color: ColorMode::Color,
            duplex: true,
            page_width_mm: 66.0,
            page_height_mm: 92.0,
            multifeed: MultifeedMode::Off,
            rotation: ScanRotation::None,
            back_first: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionOptions {
    /// Ask the id endpoints for price statistics.
    pub price_stats: bool,
    /// Read slab labels (slab_id) when the card is in a graded slab.
    pub slab: bool,
    pub grading: GradingMode,
    /// Condition scale for `GradingMode::Condition`: ebay, tcgplayer, ximilar or cardmarket.
    pub condition_mode: String,
    pub pairing: Pairing,
    /// Send each card's back to recognition with its front (sports cards and custom endpoints; needs
    /// front + back pairing).
    pub both_sides: bool,
    /// Let sport_id use Ximilar's generative model for cards it can't match (billed extra when used).
    pub magic_ai: bool,
    /// A recognition endpoint path to call instead of the standard one (empty: the standard endpoints).
    pub custom_endpoint: String,
    /// Send each image as one card covering the whole picture, so Ximilar skips its card detection (images
    /// already cropped to the card).
    pub skip_detection: bool,
    /// Assert what is known about every card, so recognition skips classifying it: its type (from the id
    /// mode), side (front, and back for the back image) and that it is upright.
    pub known_category: bool,
    pub known_side: bool,
    pub known_rotation: bool,
    /// Trading cards' writing system sent to tcg_id (`recognition::ALPHABETS`); empty = detected.
    pub alphabet: String,
    /// The game (TCG) or sport of every card (`recognition::TCG_GAMES`, `SPORTS`); empty = detected.
    pub subcategory: String,
    pub scan: ScanSettings,
    /// Mirror the session to Ximilar Cards Cloud while it runs.
    pub sync_remote: bool,
    pub engine: Engine,
    /// The local model (`crate::local::catalog`) when `engine` is `Local`.
    pub local_model: String,
}

impl Default for SessionOptions {
    fn default() -> Self {
        SessionOptions {
            price_stats: true,
            slab: false,
            grading: GradingMode::Off,
            condition_mode: "ebay".into(),
            pairing: Pairing::Single,
            both_sides: false,
            magic_ai: false,
            custom_endpoint: String::new(),
            skip_detection: false,
            known_category: false,
            known_side: false,
            known_rotation: false,
            alphabet: String::new(),
            subcategory: String::new(),
            scan: ScanSettings::default(),
            sync_remote: false,
            engine: Engine::Ximilar,
            local_model: crate::local::catalog::DEFAULT_MODEL.into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCounts {
    pub total: i64,
    pub done: i64,
    pub errors: i64,
    pub queued: i64,
    pub in_progress: i64,
    pub grading: i64,
    pub synced: i64,
    pub total_price: f64,
    pub ebay_drafts: i64,
    pub ebay_listed: i64,
    pub ebay_sold: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub name: String,
    pub source_kind: SourceKind,
    pub source_ref: String,
    pub id_mode: IdMode,
    pub options: SessionOptions,
    pub status: SessionStatus,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub remote_session_id: Option<String>,
    pub sync_error: Option<String>,
    pub ebay_error: Option<String>,
    pub ebay_checked_at: Option<String>,
    pub counts: SessionCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSession {
    pub name: String,
    pub source_kind: SourceKind,
    pub source_ref: String,
    pub id_mode: IdMode,
    pub options: SessionOptions,
}

/// The card fields of a match, named as Cards Cloud names them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardFields {
    pub name: String,
    pub full_name: String,
    pub set_name: String,
    pub set_code: String,
    pub card_number: String,
    pub series: String,
    pub year: String,
    pub subcategory: String,
}

/// What a graded slab's label says, when the card is in one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlabLabel {
    pub company: String,
    pub grade: String,
    pub certificate: String,
}

/// One row of the session table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSummary {
    pub id: i64,
    pub session_id: String,
    pub seq: i64,
    pub status: ItemStatus,
    pub front_path: String,
    pub back_path: Option<String>,
    pub thumb_path: Option<String>,
    pub id_type: Option<String>,
    #[serde(flatten)]
    pub card: CardFields,
    pub confidence: Option<String>,
    pub distance: Option<f64>,
    pub price: Option<f64>,
    pub grade: Option<f64>,
    pub condition: Option<String>,
    pub slab: Option<SlabLabel>,
    pub error: Option<String>,
    pub synced: bool,
    pub updated_at: String,
    /// The user picked the match from the candidates.
    pub picked: bool,
    /// The card's eBay listing, once one was attempted.
    pub ebay: Option<EbayListing>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EbayListing {
    /// None: nothing is on eBay yet (the first attempt failed).
    pub status: Option<EbayStatus>,
    pub price: Option<f64>,
    pub listing_id: Option<String>,
    pub error: Option<String>,
}

/// Everything the detail panel shows about one item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDetail {
    #[serde(flatten)]
    pub summary: ItemSummary,
    pub event_id: String,
    pub preview_path: Option<String>,
    pub back_preview_path: Option<String>,
    pub recognition: Option<Value>,
    pub grading: Option<Value>,
    pub grading_images: Vec<String>,
    pub calls: i64,
    pub analyzed_at: Option<String>,
    /// The row's id in Cards Cloud, once known.
    pub remote_id: Option<String>,
    pub ebay_sku: Option<String>,
    pub ebay_offer_id: Option<String>,
    /// The card's pictures uploaded to eBay.
    pub ebay_images: Vec<String>,
    /// The card's pictures on the public image host (Whatnot export).
    pub hosted_images: Vec<String>,
}

/// Cards analysed on one local day (`YYYY-MM-DD`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayCount {
    pub date: String,
    pub cards: u32,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// The derived images of a new item, or why they couldn't be made.
#[derive(Debug, Clone, Default)]
pub struct ItemImages {
    pub thumb: Option<String>,
    pub preview: Option<String>,
    pub back_preview: Option<String>,
    pub error: Option<String>,
}
