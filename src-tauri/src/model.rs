//! Types shared by the database, the runner and the UI (serialized camelCase for TypeScript).

use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! str_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $text:literal),+ $(,)? } default $default:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
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
    /// What identifies the cards: the Ximilar API, a vision language model on this computer, or an AI agent connected
    /// over MCP that reads each card's pictures (`next_cards_to_identify` / `identify_cards`).
    Engine { Ximilar = "ximilar", Local = "local", Agent = "agent" } default Ximilar
);

str_enum!(
    /// Where an identified card's price comes from: none, the price statistics Ximilar's identification returns
    /// (only when Ximilar identifies), the card's active eBay listings (Browse API, the user's keyset), or an AI
    /// agent over MCP (`next_cards_to_price` / `price_cards`). `Ebay` counts every listing the search finds;
    /// `EbayLocal` and `EbayAgent` have the local model, or the agent, keep only the listings that are the card
    /// first. Each of the three takes the median of what counts.
    Prices {
        Off = "off",
        Ximilar = "ximilar",
        Ebay = "ebay",
        Agent = "agent",
        EbayLocal = "ebay_local",
        EbayAgent = "ebay_agent",
    } default Off
);

str_enum!(
    /// What a session does with each card: identify it (and grade it, price it…), only grade it, only tell its
    /// condition, or only cut it out of the image into a folder of card pictures.
    Task { Identify = "identify", Grade = "grade", Condition = "condition", Crop = "crop" } default Identify
);

str_enum!(
    /// A step of a card's pass through a session (`SessionOptions::stages` lists them; a pass takes them in the
    /// order of `SessionOptions::pass`). A pass cut short goes on at the stage the card still owes
    /// (`items.stage`), so nothing already paid for is asked again.
    Stage { Identify = "identify", Price = "price", Grade = "grade", Crop = "crop" } default Identify
);

impl Stage {
    pub fn find(text: &str) -> Option<Self> {
        [Stage::Identify, Stage::Price, Stage::Grade, Stage::Crop]
            .into_iter()
            .find(|s| s.as_str() == text)
    }
}

str_enum!(
    /// The file type cropped pictures are saved as: JPEG (re-encoded), PNG (lossless, large) or WebP (Ximilar's own
    /// file as it comes; lossless when Cardloader cuts the card itself).
    CropFormat { Jpeg = "jpg", Png = "png", Webp = "webp" } default Jpeg
);

impl CropFormat {
    pub fn extension(self) -> &'static str {
        self.as_str()
    }
}

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
        // "Lightly Played (Excellent)": eBay's trading card steps carry the sports name in brackets.
        let t = t.split(" (").next().unwrap_or_default();
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

    /// [`CardCondition::read`] on the scale the condition was graded on (a scale id or name, e.g. `cardmarket`):
    /// Cardmarket's "Light Played" is below its "Good", where Whatnot's is the step below Near Mint.
    pub fn read_on(text: &str, scale: Option<&str>) -> Option<CardCondition> {
        let cardmarket = scale.is_some_and(|s| s.eq_ignore_ascii_case("cardmarket"));
        if cardmarket && text.trim().eq_ignore_ascii_case("light played") {
            return Some(CardCondition::VeryGood);
        }
        CardCondition::read(text)
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
    /// Identify the cards, or only grade or crop them. Neither identifies, so nothing about identification or
    /// Cards Cloud below applies; a crop session doesn't grade either.
    pub task: Task,
    /// Also save each card cropped (a crop-only session always does).
    pub save_crops: bool,
    /// Where the cropped pictures go; empty = `crops` in the session's own folder.
    pub crop_folder: String,
    /// Let Ximilar even out the levels and sharpen the crop (its `level`, on by default). Only Ximilar's crops.
    pub crop_level: bool,
    /// The file type the crops are saved as.
    pub crop_format: CropFormat,
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
    /// What identifies the cards (and, for a session that doesn't identify, what grades them unless
    /// `grade_engine` says).
    pub engine: Engine,
    /// What grades the cards; `None` (sessions made before engines were chosen per task) = `engine`.
    pub grade_engine: Option<Engine>,
    /// Where prices come from; `None` (older sessions) = Ximilar's price statistics when `price_stats`.
    pub prices: Option<Prices>,
    /// What crops the cards: Ximilar cuts them itself, the local model or an AI agent gives each card's corners
    /// and Cardloader cuts it (`crate::crop`). `None` (sessions made before crops had engines) = Ximilar,
    /// whatever `engine` is.
    pub crop_engine: Option<Engine>,
    /// The eBay site its cards are priced on (`browse::SEARCH_MARKETS`); empty = Settings' search site.
    pub ebay_site: String,
    /// The local model (`crate::local::catalog`) when identification, grading or cropping is `Local`.
    pub local_model: String,
}

impl Default for SessionOptions {
    fn default() -> Self {
        SessionOptions {
            task: Task::Identify,
            save_crops: false,
            crop_folder: String::new(),
            crop_level: true,
            crop_format: CropFormat::Jpeg,
            price_stats: true,
            slab: false,
            grading: GradingMode::Off,
            condition_mode: "ebay_tcg".into(),
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
            grade_engine: None,
            prices: None,
            crop_engine: None,
            ebay_site: String::new(),
            local_model: crate::local::catalog::DEFAULT_MODEL.into(),
        }
    }
}

impl SessionOptions {
    /// Only crops: nothing identified or graded.
    pub fn crops_only(&self) -> bool {
        self.task == Task::Crop
    }

    /// Saves cropped pictures of the cards, alone or with what else it does.
    pub fn crops(&self) -> bool {
        self.crops_only() || self.save_crops
    }

    pub fn identifies(&self) -> bool {
        self.task == Task::Identify
    }

    /// The card-grader endpoint the cards go to: a grade-only session grades (a full grade unless it asks for
    /// centering only), a condition-only session reads the condition, a crop session never grades.
    pub fn grading_mode(&self) -> GradingMode {
        match (self.task, self.grading) {
            (Task::Identify, mode) => mode,
            (Task::Grade, GradingMode::Centering) => GradingMode::Centering,
            (Task::Grade, _) => GradingMode::Grade,
            (Task::Condition, _) => GradingMode::Condition,
            (Task::Crop, _) => GradingMode::Off,
        }
    }

    /// What happens to each card, in order: its identification, its price (unless Ximilar's identification brought
    /// it), its grading (a job of the card-grader, or the local model), its crop (a job of the card-grader, or cut
    /// here at the corners the local model or an AI agent gives).
    pub fn stages(&self) -> Vec<Stage> {
        [
            (self.identifies(), Stage::Identify),
            // Ximilar's prices come with its identification: no stage of their own.
            (
                !matches!(self.prices(), Prices::Off | Prices::Ximilar),
                Stage::Price,
            ),
            (self.grading_mode() != GradingMode::Off, Stage::Grade),
            (self.crops(), Stage::Crop),
        ]
        .into_iter()
        .filter_map(|(on, stage)| on.then_some(stage))
        .collect()
    }

    /// What grades the cards. An AI agent never grades: a session it identifies grades with Ximilar.
    pub fn grade_engine(&self) -> Engine {
        match self.grade_engine.unwrap_or(self.engine) {
            Engine::Agent => Engine::Ximilar,
            engine => engine,
        }
    }

    /// How long a stage may hold a card up, which is what orders a pass: 0 identifying it (a price needs to know
    /// the card; nothing else does, but it is what the user waits for), 1 a call answered at once (Ximilar, an
    /// eBay search), 2 the model on this computer (one card at a time, tens of seconds each), 3 an AI agent
    /// (whenever it comes, if it does).
    fn wait(&self, stage: Stage) -> u8 {
        let (quick, local, agent) = (1, 2, 3);
        match stage {
            Stage::Identify => 0,
            Stage::Price => match self.prices() {
                Prices::Agent | Prices::EbayAgent => agent,
                Prices::EbayLocal => local,
                _ => quick,
            },
            Stage::Grade if self.grades_locally() => local,
            Stage::Grade => quick,
            Stage::Crop if self.crops_by_agent() => agent,
            Stage::Crop if self.crops_locally() => local,
            Stage::Crop => quick,
        }
    }

    /// `stages` in the order a card's pass takes them: identification first, then what answers at once before
    /// what the local model does, and what waits for an AI agent last. A grade asked of Ximilar is then never
    /// held up behind a price an agent hasn't given yet, and grading and crops don't need the card identified.
    pub fn in_pass_order(&self, mut stages: Vec<Stage>) -> Vec<Stage> {
        stages.sort_by_key(|s| (self.wait(*s), *s as u8));
        stages
    }

    /// The session's stages in the order a card's pass takes them (`in_pass_order`).
    pub fn pass(&self) -> Vec<Stage> {
        self.in_pass_order(self.stages())
    }

    /// Where a card owing `owed` goes on in a pass of `stages` (in pass order): at that stage, or, when the
    /// options dropped it since, at the first one left that would have followed it (none: the pass is over).
    pub fn resume(&self, stages: &[Stage], owed: Stage) -> usize {
        if let Some(at) = stages.iter().position(|s| *s == owed) {
            return at;
        }
        let mut with = stages.to_vec();
        with.push(owed);
        let with = self.in_pass_order(with);
        let after: Vec<Stage> = with
            .iter()
            .skip_while(|s| **s != owed)
            .skip(1)
            .copied()
            .collect();
        stages
            .iter()
            .position(|s| after.contains(s))
            .unwrap_or(stages.len())
    }

    /// Where the cards' prices come from. Only identified cards are priced, and Ximilar's price statistics come
    /// only with its identification.
    pub fn prices(&self) -> Prices {
        let prices = self.prices.unwrap_or(if self.price_stats {
            Prices::Ximilar
        } else {
            Prices::Off
        });
        match prices {
            _ if !self.identifies() => Prices::Off,
            Prices::Ximilar if self.engine != Engine::Ximilar => Prices::Off,
            prices => prices,
        }
    }

    /// Identified on this computer.
    pub fn identifies_locally(&self) -> bool {
        self.identifies() && self.engine == Engine::Local
    }

    /// Graded on this computer.
    pub fn grades_locally(&self) -> bool {
        self.grading_mode() != GradingMode::Off && self.grade_engine() == Engine::Local
    }

    /// What crops the cards (unless it says otherwise, Ximilar: also in a local or an agent's session).
    pub fn crop_engine(&self) -> Engine {
        self.crop_engine.unwrap_or(Engine::Ximilar)
    }

    /// Cropped by Ximilar: an async job of the card-grader.
    pub fn crops_by_ximilar(&self) -> bool {
        self.crops() && self.crop_engine() == Engine::Ximilar
    }

    /// Cut here at the corners the local model finds.
    pub fn crops_locally(&self) -> bool {
        self.crops() && self.crop_engine() == Engine::Local
    }

    /// Cut here at the corners an AI agent gives over MCP.
    pub fn crops_by_agent(&self) -> bool {
        self.crops() && self.crop_engine() == Engine::Agent
    }

    /// Priced from the eBay listings the model on this computer keeps.
    pub fn prices_locally(&self) -> bool {
        self.prices() == Prices::EbayLocal
    }

    /// Identified, graded, cropped or priced by the model on this computer.
    pub fn uses_local(&self) -> bool {
        self.identifies_locally()
            || self.grades_locally()
            || self.crops_locally()
            || self.prices_locally()
    }

    /// Identified by an AI agent over MCP. An agent identifies, prices and finds corners; it never grades.
    pub fn agent(&self) -> bool {
        self.engine == Engine::Agent && self.identifies()
    }

    /// Priced by an AI agent over MCP: on its own, or from the eBay listings Cardloader found for it.
    pub fn prices_by_agent(&self) -> bool {
        matches!(self.prices(), Prices::Agent | Prices::EbayAgent)
    }

    /// The agent that prices the cards gets each card's eBay listings and says which are the card; the price is
    /// Cardloader's (their median), not the agent's.
    pub fn agent_picks_listings(&self) -> bool {
        self.prices() == Prices::EbayAgent
    }

    /// Prices the cards from their active eBay listings, whoever says which of them count.
    pub fn uses_ebay(&self) -> bool {
        matches!(
            self.prices(),
            Prices::Ebay | Prices::EbayLocal | Prices::EbayAgent
        )
    }

    /// Calls the Ximilar API for something: identifying, grading, cropping or syncing. A session whose stages all
    /// run elsewhere runs without a token.
    pub fn uses_api(&self) -> bool {
        self.crops_by_ximilar()
            || (self.identifies() && (self.sync_remote || self.engine == Engine::Ximilar))
            || (self.grading_mode() != GradingMode::Off && self.grade_engine() == Engine::Ximilar)
    }
}

/// An amount in one currency (ISO 4217): totals of prices in different currencies are kept apart.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Money {
    pub currency: String,
    pub total: f64,
}

/// Prices, grading or crops added to a session's identified cards after its run (`AppState::add_to_cards`): what
/// to add, and by what.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AddToCards {
    /// From eBay's listings or by an AI agent (Ximilar's come only with its identification).
    pub prices: Option<Prices>,
    /// A full grade, centering, or the condition.
    pub grading: Option<GradingMode>,
    /// Ximilar or the local model; default: the session's.
    pub grade_engine: Option<Engine>,
    /// The condition scale (`tcgplayer`, `cardmarket`, `ebay_tcg`, …); default: the session's.
    pub condition_mode: Option<String>,
    /// The local model that grades or crops; default: the session's.
    pub local_model: Option<String>,
    /// Save each card cropped.
    pub crops: bool,
    /// Ximilar, the local model or an AI agent; default: the session's.
    pub crop_engine: Option<Engine>,
    /// Where the crops go; default: the session's (empty = `crops` in its own folder).
    pub crop_folder: Option<String>,
    /// Default: the session's.
    pub crop_format: Option<CropFormat>,
    /// Ximilar's levels and sharpening; default: the session's.
    pub crop_level: Option<bool>,
    /// The eBay site prices come from (`EBAY_DE`, …); default: the session's.
    pub ebay_site: Option<String>,
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
    /// The cards' prices in US dollars (`values` has every currency).
    pub total_price: f64,
    /// The cards' prices by currency, the largest first.
    pub values: Vec<Money>,
    pub ebay_drafts: i64,
    pub ebay_listed: i64,
    pub ebay_sold: i64,
    /// Identified cards nobody has accepted or corrected yet.
    pub to_review: i64,
    /// Paid Ximilar API calls the cards took.
    pub calls: i64,
    /// Identified cards in their price stage (searching eBay, or waiting for an AI agent's price).
    pub pricing: i64,
    /// Cards in their crop stage: being cut here, or waiting for an AI agent's corners.
    pub cropping: i64,
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
    /// Who made the card ("Topps"). This and the three below are details of the card, not what tells it from
    /// another: Cards Cloud doesn't take them, and typing one leaves the review as it was.
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub team: String,
    /// The copy's serial number as printed ("43/75").
    #[serde(default)]
    pub serial: String,
    /// What a seller says about the card in a title ("RC Auto"): only ever typed, by the user, a CSV or an agent.
    #[serde(default)]
    pub features: String,
}

impl CardFields {
    pub fn trim(&mut self) {
        for field in [
            &mut self.name,
            &mut self.full_name,
            &mut self.set_name,
            &mut self.set_code,
            &mut self.card_number,
            &mut self.series,
            &mut self.year,
            &mut self.subcategory,
            &mut self.brand,
            &mut self.team,
            &mut self.serial,
            &mut self.features,
        ] {
            *field = field.trim().to_string();
        }
    }

    /// "Name Set #number" from the fields there are.
    pub fn composed_full_name(&self) -> String {
        let number = (!self.card_number.is_empty()).then(|| format!("#{}", self.card_number));
        [Some(self.name.clone()), Some(self.set_name.clone()), number]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// One part of a listing title (`ebay::title`). A name this version doesn't know (a file from a newer one) reads
/// as `Unknown` and is left out: a settings file that doesn't parse would lose every setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TitleField {
    Year,
    /// The game, or the sport.
    Game,
    Brand,
    Set,
    SetCode,
    Name,
    /// The variant, parallel or insert (`CardFields::series`).
    Series,
    Features,
    Serial,
    /// `#125`.
    Number,
    Team,
    /// A slab's grader and grade: `PSA 9.5`.
    Grade,
    #[serde(other)]
    Unknown,
}

/// The kind of card a title template is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TitleKind {
    #[default]
    Any,
    Tcg,
    Sport,
    #[serde(other)]
    Unknown,
}

/// Which of those cards a title template is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TitleOnly {
    #[default]
    Any,
    /// Cards in a slab whose label was read.
    Graded,
    /// Cards whose features say so ("Auto", "Autograph").
    Autograph,
    #[serde(other)]
    Unknown,
}

/// The parts of a listing title, in the user's order, for the cards it is for. The first template in the settings
/// that fits a card makes its title.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TitleTemplate {
    pub id: String,
    pub name: String,
    pub kind: TitleKind,
    pub only: TitleOnly,
    pub fields: Vec<TitleField>,
}

/// What a change to many cards at once writes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditField {
    /// The card's own listing title.
    Title,
    Features,
    Brand,
    Team,
    Series,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditPlace {
    Start,
    End,
    /// Instead of what is there; with no text, the field is emptied (a title: automatic again).
    Replace,
}

/// One change for many cards: a text added to (or put in place of) one of their fields or their titles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkEdit {
    pub field: EditField,
    pub place: EditPlace,
    pub text: String,
}

/// Whose titles to show (`AppState::preview_titles`): the cards named, else the newest identified ones, else
/// made-up ones.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TitleQuery {
    pub item_ids: Option<Vec<i64>>,
    /// A change not made yet: the titles are the ones it would give.
    pub edit: Option<BulkEdit>,
    /// Templates not saved yet, instead of the stored ones.
    pub templates: Option<Vec<TitleTemplate>>,
}

/// A card's listing title, now and after a change.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitlePreview {
    /// None: a made-up card, shown while there are no cards yet.
    pub item_id: Option<i64>,
    pub seq: i64,
    pub thumb_path: Option<String>,
    /// The title as it is.
    pub before: String,
    /// The title after the change asked about; without one, the same.
    pub title: String,
    /// `title` is the card's own, not a template's.
    pub custom: bool,
    /// What the template makes for the card, whether or not it has its own title.
    pub automatic: String,
    /// The template that makes it; empty = the built-in one.
    pub template: String,
    /// Parts the card has that didn't fit.
    pub left_out: Vec<TitleField>,
    /// The change asked about changes this card.
    pub changed: bool,
    /// Why the card has no listing title, or can't take the change.
    pub problem: Option<String>,
}

/// What a change to many cards did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditedItems {
    pub changed: usize,
    /// Cards that couldn't take it (their title would be too long, they aren't identified).
    pub skipped: usize,
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
    /// The price's currency (ISO 4217): USD unless it came from another eBay site, or an agent or CSV said so.
    #[serde(default = "usd")]
    pub price_currency: String,
    pub grade: Option<f64>,
    pub condition: Option<String>,
    pub slab: Option<SlabLabel>,
    pub error: Option<String>,
    pub synced: bool,
    pub updated_at: String,
    /// The user picked the match from the candidates.
    pub picked: bool,
    /// The user typed the card's fields or price.
    pub edited: bool,
    /// The user accepted the match (or corrected it).
    pub reviewed: bool,
    /// An AI agent (over MCP) accepted, picked or corrected it, not the user.
    pub agent_reviewed: bool,
    /// What the agent said about the card when it did.
    pub review_note: String,
    pub notes: String,
    /// Where the card is kept: a box, a binder page.
    pub location: String,
    /// The listing title someone wrote for this card; empty = the one its template makes (`ebay::title`).
    #[serde(default)]
    pub title: String,
    /// The card's eBay listing, once one was attempted.
    pub ebay: Option<EbayListing>,
    /// What its active eBay listings ask, once they were searched.
    pub market_range: Option<PriceRange>,
}

impl ItemSummary {
    /// The card's price when it is in US dollars: what TCGplayer, Whatnot, Shopify, eBay US and Cards Cloud are
    /// given. A price in another currency (another eBay site's) is left for the user to set there.
    pub fn usd_price(&self) -> Option<f64> {
        self.price.filter(|_| self.price_currency == USD)
    }
}

/// What active eBay listings of a card ask, over those that are the card (see [`EbayMarket`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceRange {
    pub min: f64,
    pub max: f64,
    pub average: f64,
    pub median: f64,
    pub count: i64,
    pub currency: String,
}

impl PriceRange {
    /// The range of the listings that count, in the currency most of them ask in; none without a price.
    pub fn of(listings: &[MarketListing]) -> Option<PriceRange> {
        let counted: Vec<&MarketListing> = listings.iter().filter(|l| l.counted).collect();
        let mut currencies: Vec<&str> = counted.iter().map(|l| l.currency.as_str()).collect();
        currencies.sort_unstable();
        let currency = currencies
            .chunk_by(|a, b| a == b)
            .max_by_key(|run| run.len())?
            .first()?
            .to_string();
        let mut prices: Vec<f64> = counted
            .iter()
            .filter(|l| l.currency == currency && l.price.is_finite())
            .map(|l| l.price)
            .collect();
        prices.sort_by(|a, b| a.total_cmp(b));
        let n = prices.len();
        if n == 0 {
            return None;
        }
        let median = if n % 2 == 1 {
            prices[n / 2]
        } else {
            (prices[n / 2 - 1] + prices[n / 2]) / 2.0
        };
        let cents = |v: f64| (v * 100.0).round() / 100.0;
        Some(PriceRange {
            min: prices[0],
            max: prices[n - 1],
            average: cents(prices.iter().sum::<f64>() / n as f64),
            median: cents(median),
            count: n as i64,
            currency,
        })
    }
}

/// One active eBay listing found for a card.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketListing {
    /// eBay's item id (`v1|…|0`).
    pub id: String,
    pub title: String,
    /// The asking price, or an auction's current bid.
    pub price: f64,
    pub currency: String,
    pub url: String,
    pub image: Option<String>,
    pub condition: String,
    /// FIXED_PRICE, AUCTION, BEST_OFFER.
    pub buying: Vec<String>,
    pub seller: String,
    pub country: String,
    /// It is this card: its price counts in the range (all do until an agent or the user keeps only some).
    pub counted: bool,
}

/// A card's active listings on eBay, as last searched, and the range of those that are the card.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EbayMarket {
    pub query: String,
    /// The eBay site searched (`EBAY_DE`, …).
    #[serde(default = "us_site")]
    pub marketplace: String,
    pub category_id: Option<String>,
    pub searched_at: String,
    /// How many listings eBay has for the query: more than are kept here.
    pub total: i64,
    pub listings: Vec<MarketListing>,
    /// Why only some listings count (an agent's note), if it chose.
    pub note: String,
    pub range: Option<PriceRange>,
}

/// Who accepts, picks or corrects a card: the user in the app, or an AI agent over MCP saying why.
#[derive(Debug, Clone, PartialEq)]
pub enum Reviewer {
    User,
    Agent { note: String },
}

impl Reviewer {
    pub fn is_agent(&self) -> bool {
        matches!(self, Reviewer::Agent { .. })
    }

    /// The agent's note; none for the user.
    pub fn note(&self) -> Option<&str> {
        match self {
            Reviewer::User => None,
            Reviewer::Agent { note } => Some(note.trim()).filter(|n| !n.is_empty()),
        }
    }
}

/// A card on the Cards page: an identified card of any session, with how many such cards there are.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRow {
    #[serde(flatten)]
    pub item: ItemSummary,
    pub session_name: String,
    /// Identified cards with the same name, set and number, this one included.
    pub copies: i64,
}

/// The cards matching a search: the first `limit` and the totals of all of them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSearch {
    pub cards: Vec<CardRow>,
    pub total: i64,
    /// The matches' prices in US dollars (`values` has every currency).
    pub value: f64,
    /// The matches' prices by currency, the largest first.
    pub values: Vec<Money>,
}

/// What the user can change on a card by hand.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemEdit {
    #[serde(flatten)]
    pub card: CardFields,
    pub price: Option<f64>,
    pub notes: String,
    pub location: String,
    /// The card's own listing title; empty = automatic.
    #[serde(default)]
    pub title: String,
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
    /// Its crop job's result (`{"endpoint": "crop_level", "response": …}`) and the cropped pictures saved.
    pub crop: Option<Value>,
    pub crop_images: Vec<String>,
    /// Its active eBay listings, once searched.
    pub market: Option<EbayMarket>,
    /// Why the AI agent that priced the card chose its price.
    pub price_note: Option<String>,
}

/// Cards analysed on one local day (`YYYY-MM-DD`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayCount {
    pub date: String,
    pub cards: u32,
    /// Paid Ximilar API calls of the cards analysed that day.
    pub calls: u32,
}

/// The currency prices are in unless they say otherwise: Ximilar's, and every price before currencies were kept.
pub const USD: &str = "USD";

fn usd() -> String {
    USD.into()
}

fn us_site() -> String {
    "EBAY_US".into()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_task_grades_its_own_way() {
        let with = |task, grading| {
            SessionOptions {
                task,
                grading,
                ..SessionOptions::default()
            }
            .grading_mode()
        };
        assert_eq!(with(Task::Identify, GradingMode::Off), GradingMode::Off);
        assert_eq!(
            with(Task::Identify, GradingMode::Condition),
            GradingMode::Condition
        );
        assert_eq!(with(Task::Grade, GradingMode::Off), GradingMode::Grade);
        assert_eq!(
            with(Task::Grade, GradingMode::Condition),
            GradingMode::Grade
        );
        assert_eq!(
            with(Task::Grade, GradingMode::Centering),
            GradingMode::Centering
        );
        assert_eq!(
            with(Task::Condition, GradingMode::Grade),
            GradingMode::Condition
        );
        assert_eq!(with(Task::Crop, GradingMode::Grade), GradingMode::Off);
    }

    #[test]
    fn each_task_has_its_stages() {
        use Stage::*;
        let with = |task, grading, save_crops| {
            SessionOptions {
                task,
                grading,
                save_crops,
                ..SessionOptions::default()
            }
            .stages()
        };
        assert_eq!(with(Task::Identify, GradingMode::Off, false), [Identify]);
        assert_eq!(
            with(Task::Identify, GradingMode::Grade, true),
            [Identify, Grade, Crop]
        );
        assert_eq!(with(Task::Grade, GradingMode::Off, false), [Grade]);
        assert_eq!(with(Task::Condition, GradingMode::Off, true), [Grade, Crop]);
        assert_eq!(with(Task::Crop, GradingMode::Grade, false), [Crop]);
        let priced = |prices| SessionOptions {
            prices: Some(prices),
            grading: GradingMode::Grade,
            ..SessionOptions::default()
        };
        assert_eq!(priced(Prices::Ebay).stages(), [Identify, Price, Grade]);
        assert_eq!(
            priced(Prices::Ximilar).stages(),
            [Identify, Grade],
            "Ximilar's prices come with its identification"
        );
        let o = SessionOptions::default();
        assert_eq!(o.resume(&[Identify, Price, Grade], Price), 1);
        assert_eq!(
            o.resume(&[Identify, Grade], Price),
            1,
            "prices dropped since"
        );
        assert_eq!(Stage::find("price"), Some(Price));

        // A card owing a stage goes on there; when the options dropped the stage since, at the next one left.
        assert_eq!(o.resume(&[Identify, Grade, Crop], Grade), 1);
        assert_eq!(o.resume(&[Identify, Crop], Grade), 1);
        assert_eq!(o.resume(&[Crop], Crop), 0);
        assert_eq!(o.resume(&[Identify], Crop), 1, "nothing left to do");
    }

    /// A pass takes a card's stages by how long each may hold it up: identified first, then what answers at
    /// once, then the local model's work, then what waits for an AI agent.
    #[test]
    fn a_pass_takes_the_quick_stages_before_the_slow_ones() {
        use Stage::*;
        let all = |prices, grade_engine, crop_engine| SessionOptions {
            prices: Some(prices),
            grading: GradingMode::Grade,
            grade_engine: Some(grade_engine),
            save_crops: true,
            crop_engine: Some(crop_engine),
            ..SessionOptions::default()
        };
        // All by Ximilar and eBay: the order they are listed in.
        let quick = all(Prices::Ebay, Engine::Ximilar, Engine::Ximilar);
        assert_eq!(quick.pass(), [Identify, Price, Grade, Crop]);
        assert_eq!(quick.pass(), quick.stages());
        // A price that waits for an agent comes last: the grade and the crop don't wait for it.
        let agent_prices = all(Prices::EbayAgent, Engine::Ximilar, Engine::Ximilar);
        assert_eq!(agent_prices.pass(), [Identify, Grade, Crop, Price]);
        assert_eq!(agent_prices.stages(), [Identify, Price, Grade, Crop]);
        // The local model's listings after Ximilar's grade, before the corners an agent finds.
        let mixed = all(Prices::EbayLocal, Engine::Ximilar, Engine::Agent);
        assert_eq!(mixed.pass(), [Identify, Grade, Price, Crop]);
        let local = all(Prices::Ebay, Engine::Local, Engine::Ximilar);
        assert_eq!(local.pass(), [Identify, Price, Crop, Grade]);
        // Identifying is first even when an agent does it.
        let by_agent = SessionOptions {
            engine: Engine::Agent,
            ..all(Prices::Ebay, Engine::Ximilar, Engine::Ximilar)
        };
        assert_eq!(by_agent.pass()[0], Identify);

        // A card goes on at the stage it owes, in that order.
        let pass = agent_prices.pass();
        assert_eq!(agent_prices.resume(&pass, Crop), 2);
        assert_eq!(agent_prices.resume(&pass, Price), 3);
        // The options dropped the crops since: what would have followed them is the price.
        let dropped = SessionOptions {
            save_crops: false,
            ..agent_prices.clone()
        };
        assert_eq!(dropped.pass(), [Identify, Grade, Price]);
        assert_eq!(dropped.resume(&dropped.pass(), Crop), 2);
        // Prices added to finished cards alone (`only_stages`): nothing left once they are dropped.
        assert_eq!(agent_prices.resume(&[Grade], Price), 1);
    }

    /// What a session's options mean is written twice, here and in `src/lib/format.ts` (`gradingOf`, `usesApi`,
    /// `agentOf`, `cropsOf`, `cropEngineOf`). Both are checked against the same cases, so neither drifts from the other.
    #[test]
    fn options_mean_what_the_shared_cases_say() {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Case {
            options: SessionOptions,
            grading_mode: GradingMode,
            uses_api: bool,
            agent: bool,
            crops: bool,
            grade_engine: Engine,
            crop_engine: Engine,
            prices: Prices,
            stages: Vec<Stage>,
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../src/lib/session-options.cases.json")).unwrap();
        assert!(cases.len() > 10);
        for case in cases {
            let o = &case.options;
            let what = format!(
                "{:?} by {:?}, grading {:?} by {:?}, prices {:?}, crops {} by {:?}, sync {}",
                o.task,
                o.engine,
                o.grading,
                o.grade_engine,
                o.prices,
                o.save_crops,
                o.crop_engine,
                o.sync_remote
            );
            assert_eq!(o.grading_mode(), case.grading_mode, "{what}");
            assert_eq!(o.uses_api(), case.uses_api, "{what}");
            assert_eq!(o.agent(), case.agent, "{what}");
            assert_eq!(o.crops(), case.crops, "{what}");
            assert_eq!(o.grade_engine(), case.grade_engine, "{what}");
            assert_eq!(o.crop_engine(), case.crop_engine, "{what}");
            assert_eq!(o.prices(), case.prices, "{what}");
            assert_eq!(o.stages(), case.stages, "{what}");
        }
    }

    #[test]
    fn what_needs_the_api() {
        let with = |engine, grading, save_crops| SessionOptions {
            engine,
            grading,
            save_crops,
            ..SessionOptions::default()
        };
        assert!(with(Engine::Ximilar, GradingMode::Off, false).uses_api());
        assert!(!with(Engine::Local, GradingMode::Grade, false).uses_api());
        assert!(!with(Engine::Agent, GradingMode::Off, false).uses_api());
        // The agent identifies; Ximilar grades and crops.
        assert!(with(Engine::Agent, GradingMode::Grade, false).uses_api());
        assert!(with(Engine::Agent, GradingMode::Off, true).uses_api());
        let grade_only = SessionOptions {
            task: Task::Grade,
            ..with(Engine::Agent, GradingMode::Off, false)
        };
        assert!(!grade_only.agent());
        assert!(grade_only.uses_api());

        // Each task by its own engine: identified by Ximilar and graded here, or the other way round.
        let mixed = |engine, grade_engine| SessionOptions {
            grade_engine: Some(grade_engine),
            ..with(engine, GradingMode::Grade, false)
        };
        let ximilar_then_local = mixed(Engine::Ximilar, Engine::Local);
        assert!(ximilar_then_local.grades_locally() && !ximilar_then_local.identifies_locally());
        assert!(ximilar_then_local.uses_api() && ximilar_then_local.uses_local());
        let local_then_ximilar = mixed(Engine::Local, Engine::Ximilar);
        assert!(local_then_ximilar.identifies_locally() && !local_then_ximilar.grades_locally());
        assert!(local_then_ximilar.uses_api());
        assert!(!mixed(Engine::Local, Engine::Local).uses_api());
        assert_eq!(
            mixed(Engine::Agent, Engine::Agent).grade_engine(),
            Engine::Ximilar,
            "an agent never grades"
        );
        let agent_prices = SessionOptions {
            prices: Some(Prices::Agent),
            ..with(Engine::Local, GradingMode::Off, false)
        };
        assert!(
            agent_prices.prices_by_agent() && !agent_prices.agent() && !agent_prices.uses_api()
        );
        assert!(!agent_prices.uses_ebay() && !agent_prices.agent_picks_listings());
        // eBay's listings, with the local model or an agent saying which are the card.
        let picked_here = SessionOptions {
            prices: Some(Prices::EbayLocal),
            ..with(Engine::Agent, GradingMode::Off, false)
        };
        assert!(
            picked_here.uses_ebay() && picked_here.uses_local() && picked_here.prices_locally()
        );
        assert!(!picked_here.prices_by_agent() && !picked_here.uses_api());
        let picked_by_agent = SessionOptions {
            prices: Some(Prices::EbayAgent),
            ..picked_here.clone()
        };
        assert!(picked_by_agent.uses_ebay() && !picked_by_agent.uses_local());
        assert!(picked_by_agent.prices_by_agent() && picked_by_agent.agent_picks_listings());
        assert_eq!(
            picked_by_agent.stages(),
            vec![Stage::Identify, Stage::Price]
        );

        // Crops are Ximilar's unless they say otherwise, whatever identifies; cut here, they need no token.
        let crops = |engine, crop_engine| SessionOptions {
            crop_engine,
            ..with(engine, GradingMode::Off, true)
        };
        assert!(crops(Engine::Local, None).crops_by_ximilar());
        let cut_here = crops(Engine::Local, Some(Engine::Local));
        assert!(cut_here.crops_locally() && cut_here.uses_local() && !cut_here.uses_api());
        let by_agent = crops(Engine::Local, Some(Engine::Agent));
        assert!(by_agent.crops_by_agent() && !by_agent.agent());
        assert!(!by_agent.uses_api());
        assert!(crops(Engine::Ximilar, Some(Engine::Agent)).uses_api());
        let crop_only = SessionOptions {
            task: Task::Crop,
            ..crops(Engine::Ximilar, Some(Engine::Agent))
        };
        assert!(crop_only.crops_by_agent() && !crop_only.uses_api());
        // No crops: the crop engine means nothing.
        let none = SessionOptions {
            crop_engine: Some(Engine::Local),
            ..with(Engine::Ximilar, GradingMode::Off, false)
        };
        assert!(!none.uses_local() && !none.crops_locally());
    }
}
