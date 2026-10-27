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
use crate::ebay::listing::CardCondition;
use crate::ebay::sync::DraftItem;
use crate::error::{Error, Result};
use crate::imaging;
use crate::market::{shopify, whatnot, PricedItem};
use crate::model::*;
use crate::settings::{McpSettings, Settings};
use crate::source::{self, csv_file};
use crate::ximilar::recognition::{self, ALPHABETS, SPORTS, TCG_GAMES};

/// What the server tells agents up front.
const INSTRUCTIONS: &str = "Cardloader identifies trading cards, sports cards and comics from images with Ximilar's \
API. A session is a folder of images (or a CSV) analysed card by card; each card gets a best match and other \
candidates. Every identified card waits for review until it is accepted. Wherever a session id is asked for, \
\"latest\" means the newest session. Typical work: inspect_folder → create_session → wait_for_session (repeat \
until finished) → list_cards with filter to_review → get_card (look at the picture) → accept_cards or \
select_candidate. Cards you accept are marked as accepted by AI in the app, with your note; the user sees \
everything you do live. Tools you may not use are left out: the user allows them in Cardloader → Settings → AI \
agents.";

/// The long edge of card pictures sent to agents: enough to read the name and number, few tokens.
const PICTURE_LONG_EDGE: u32 = 1024;
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

#[derive(Deserialize, JsonSchema)]
struct SessionArg {
    /// The session id, or "latest" for the newest session.
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
    /// The session id, or "latest".
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
    /// Also send each card's back to identification (sports cards; needs front_back).
    read_backs: Option<bool>,
    /// The images are known to be fronts (and backs, when paired): faster and more accurate.
    known_side: Option<bool>,
    /// The images are known to be upright: faster and more accurate.
    upright: Option<bool>,
    /// The images are already cropped to the card.
    cropped: Option<bool>,
    /// Grade the cards' condition after identifying them (billed extra).
    grading: Option<GradingMode>,
    /// Mirror the session to Ximilar Cards Cloud ("store it in Ximilar"). Default: the user's default.
    store_in_cards_cloud: Option<bool>,
    /// A Cards Cloud session id or link to continue: files whose cards it has are skipped.
    continue_cards_cloud_session: Option<String>,
    /// Ask for market prices. Default: the user's default.
    price_stats: Option<bool>,
    /// CSV only: also analyse rows that are already identified.
    reanalyse: Option<bool>,
    /// Start analysing right away (default true).
    start: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct RunSession {
    /// The session id, or "latest".
    session_id: String,
    /// Only analyse the cards waiting in the queue: no new files, no scan (default false).
    only_queued: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct WaitForSession {
    /// The session id, or "latest".
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
    /// The session id, or "latest".
    session_id: String,
    /// Default: every card that failed.
    item_ids: Option<Vec<i64>>,
}

#[derive(Deserialize, JsonSchema)]
struct ItemIds {
    item_ids: Vec<i64>,
}

#[derive(Deserialize, JsonSchema)]
struct SetLocation {
    item_ids: Vec<i64>,
    /// e.g. "Box 3, row 2"; empty clears it.
    location: String,
}

#[derive(Deserialize, JsonSchema)]
struct ExportCsv {
    /// The session id, or "latest".
    session_id: String,
    /// Where to write the file (absolute path).
    path: String,
    /// Replace the file if it exists (default false).
    overwrite: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ExportTcgplayer {
    /// The session id, or "latest".
    session_id: String,
    path: String,
    /// For cards whose condition wasn't graded (default near_mint).
    condition: Option<CardCondition>,
    /// The cards and their prices. Default: every identified card at its market price.
    items: Option<Vec<PricedItem>>,
    overwrite: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct DeleteCards {
    /// The session id, or "latest".
    session_id: String,
    item_ids: Vec<i64>,
}

#[derive(Deserialize, JsonSchema)]
struct DeleteSession {
    /// The session id ("latest" is not accepted here).
    session_id: String,
    /// Also delete its copy in Cards Cloud (default false).
    also_in_cards_cloud: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct EbayDrafts {
    /// The session id, or "latest".
    session_id: String,
    /// The cards and their prices. Default: every identified card that has a market price, at that price.
    items: Option<Vec<DraftItem>>,
    /// For cards whose condition wasn't graded (default near_mint).
    condition: Option<CardCondition>,
}

#[derive(Deserialize, JsonSchema)]
struct EbayCards {
    /// The session id, or "latest".
    session_id: String,
    /// Default: all of the session's cards that apply.
    item_ids: Option<Vec<i64>>,
}

#[derive(Deserialize, JsonSchema)]
struct EbayRefresh {
    /// The session id, or "latest".
    session_id: String,
    /// Read the listings even if they were read a few minutes ago.
    force: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ExportWhatnot {
    /// The session id, or "latest".
    session_id: String,
    path: String,
    options: whatnot::Options,
    /// Default: every identified card at its market price.
    items: Option<Vec<PricedItem>>,
    overwrite: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ExportShopify {
    /// The session id, or "latest".
    session_id: String,
    path: String,
    options: shopify::Options,
    /// Default: every identified card at its market price.
    items: Option<Vec<PricedItem>>,
    overwrite: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct UpdateDefaults {
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
        spec::<WaitForSession>(Run, "wait_for_session", "Wait for a session", "Waits until the session's run ends or the timeout passes, and returns its progress. Call it again while it still runs."),
        spec::<AcceptCards>(Run, "accept_cards", "Accept cards", "Accepts the cards' matches (or takes that back with accepted false). They are marked as accepted by AI with your note. Cards the user accepted stay theirs."),
        spec::<SelectCandidate>(Run, "select_candidate", "Select a candidate", "Makes another candidate the card's match (get_card lists them by index); the card counts as accepted."),
        spec::<EditCard>(Run, "edit_card", "Edit a card", "Corrects a card's fields or price by hand, or sets its notes and location; given fields change, others stay."),
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
            "accept_cards" => self.accept(args(a)?),
            "select_candidate" => {
                let a: SelectCandidate = args(a)?;
                let by = agent(a.note);
                self.state.runner.pick_match(a.item_id, a.index, &by)?;
                self.card(a.item_id, false).await
            }
            "edit_card" => self.edit(args(a)?).await,
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
                let path = writable(&a.path, a.overwrite)?;
                let items = self.state.db.items(&id)?;
                crate::export::write_csv(&items, &path)?;
                out(json!({"path": path, "cards": items.len()}))
            }
            "export_tcgplayer" => {
                let a: ExportTcgplayer = args(a)?;
                let id = self.session_id(&a.session_id)?;
                let path = writable(&a.path, a.overwrite)?;
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
                            i.price.filter(|p| *p > 0.0).map(|price| DraftItem {
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
                let path = writable(&a.path, a.overwrite)?;
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
                let path = writable(&a.path, a.overwrite)?;
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
    fn session_id(&self, given: &str) -> Result<String> {
        let given = given.trim();
        if given.is_empty() || given.eq_ignore_ascii_case("latest") {
            return self
                .state
                .db
                .sessions()?
                .into_iter()
                .next()
                .map(|s| s.id)
                .ok_or_else(|| Error::msg("there are no sessions yet"));
        }
        Ok(self.state.db.session(given)?.id)
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
                "apiCalls": c.calls,
            },
        });
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
        v["candidates"] = json!(recognition::candidates(response));
        v["rotation"] = json!(recognition::rotation(response));
        v["links"] = json!(endpoint.map(|e| recognition::parse(e, response).links));
        v["files"] = json!({"front": item.summary.front_path, "back": item.summary.back_path});
        v["analyzedAt"] = json!(item.analyzed_at);
        let mut content = out(v)?;
        if images {
            let s = &item.summary;
            let sides = [
                (
                    "Front",
                    item.preview_path.clone().or(Some(s.front_path.clone())),
                ),
                (
                    "Back",
                    item.back_preview_path.clone().or(s.back_path.clone()),
                ),
            ];
            for (label, path) in sides {
                let Some(path) = path.filter(|p| !p.is_empty()) else {
                    continue;
                };
                let picture = tokio::task::spawn_blocking(move || {
                    imaging::encode_for_upload(Path::new(&path), PICTURE_LONG_EDGE, 85)
                })
                .await
                .map_err(|e| Error::msg(e.to_string()))?;
                match picture {
                    Ok(data) => {
                        content.push(ContentBlock::text(format!("{label} of card {item_id}:")));
                        content.push(ContentBlock::image(data, "image/jpeg"));
                    }
                    Err(e) => {
                        content.push(ContentBlock::text(format!("{label}: no picture ({e})")))
                    }
                }
            }
        }
        Ok(content)
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
        out(json!({"matching": found.total, "value": found.value, "cards": cards}))
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
        let options = SessionOptions {
            price_stats: a.price_stats.unwrap_or(settings.price_stats),
            grading: a.grading.unwrap_or(settings.default_grading),
            condition_mode: settings.default_condition_mode.clone(),
            pairing: a.pairing.unwrap_or(settings.default_pairing),
            both_sides: a.read_backs.unwrap_or(false),
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
            engine: settings.engine,
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
        let mut v = self.session_json(&self.state.db.session(&session.id)?, true);
        if start {
            v["next"] = json!("call wait_for_session until it finishes");
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
                let status = self.state.db.item_summary(id)?.status;
                if matches!(status, ItemStatus::Done | ItemStatus::Grading) {
                    identified.push(id);
                } else {
                    skipped.push(json!({"id": id, "why": format!("{} cards can't be accepted", status.as_str())}));
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
        };
        self.state
            .runner
            .edit_item(a.item_id, edit, &agent(a.note))?;
        self.card(a.item_id, false).await
    }

    fn update_defaults(&self, a: UpdateDefaults) -> Result<Vec<ContentBlock>> {
        let mut s = self.state.settings.get();
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
                    price: i.price.unwrap_or(0.0),
                })
                .collect()),
        }
    }
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
        CardFilter::ToReview => identified && !i.reviewed,
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
        "type": i.id_type,
        "confidence": i.confidence,
        "distance": i.distance,
        "price": i.price,
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
    })
}

fn defaults(s: &Settings) -> Value {
    json!({
        "cardType": s.default_id_mode,
        "game": s.default_subcategory,
        "grading": s.default_grading,
        "pairing": s.default_pairing,
        "conditionScale": s.default_condition_mode,
        "storeInCardsCloud": s.sync_by_default,
        "priceStats": s.price_stats,
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
    })
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

/// Where an export may write: an absolute path in an existing folder, not an existing file unless allowed.
fn writable(path: &str, overwrite: Option<bool>) -> Result<PathBuf> {
    let path = expand(path);
    if !path.is_absolute() {
        return Err(Error::msg("give the file's absolute path"));
    }
    if !path.parent().is_some_and(Path::is_dir) {
        return Err(Error::msg(format!("no such folder: {}", path.display())));
    }
    if path.exists() && !overwrite.unwrap_or(false) {
        return Err(Error::msg(format!(
            "{} exists; pass overwrite true to replace it",
            path.display()
        )));
    }
    Ok(path)
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
