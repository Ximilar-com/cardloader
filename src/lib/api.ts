// Typed wrappers around the Rust commands (src-tauri/src/commands.rs) and the shapes they return
// (src-tauri/src/model.rs, serialized camelCase).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type SourceKind = "folder" | "watch_folder" | "scanner" | "csv";

/** What importing a CSV would bring in. */
export interface CsvPreview {
  rows: number;
  /** Rows naming a card: imported as identified cards. */
  cards: number;
  cardsWithImages: number;
  /** Rows with only an image: analysed when the session runs. */
  images: number;
  skipped: number;
  missingImages: number;
  /** "Header → field" for the understood columns. */
  columns: string[];
}
export type IdMode = "auto" | "tcg" | "sport" | "comics" | "slab";
/** What identifies the cards: the Ximilar API or a vision language model on this computer. */
export type Engine = "ximilar" | "local";
export type GradingMode = "off" | "condition" | "grade" | "centering";
export type Pairing = "single" | "front_back";
export type SessionStatus = "draft" | "running" | "paused" | "stopped" | "completed" | "failed";
export type ItemStatus = "queued" | "recognizing" | "grading" | "done" | "error";
export type ColorMode = "color" | "gray" | "bw";
export type MultifeedMode = "off" | "default";
export type ScanRotation = "none" | "upside_down";

export interface ScanSettings {
  dpi: number;
  color: ColorMode;
  duplex: boolean;
  pageWidthMm: number;
  pageHeightMm: number;
  multifeed: MultifeedMode;
  rotation: ScanRotation;
  /** Duplex: the scanner's first side of each card is the card's back. */
  backFirst: boolean;
}

export interface SessionOptions {
  priceStats: boolean;
  slab: boolean;
  grading: GradingMode;
  conditionMode: string;
  pairing: Pairing;
  /** Send each card's back to recognition with its front (sport_id and custom endpoints). */
  bothSides: boolean;
  /** sport_id's generative fallback for cards outside Ximilar's database. */
  magicAi: boolean;
  /** A recognition endpoint path to call instead of the standard one; empty for the standard endpoints. */
  customEndpoint: string;
  /** Send each image as one card over the whole picture, so Ximilar skips detecting it. */
  skipDetection: boolean;
  /** Assert every card's type (from the id mode), side (front; back for the back image) and that it is upright. */
  knownCategory: boolean;
  knownSide: boolean;
  knownRotation: boolean;
  /** Trading cards' writing system for tcg_id ("latin", "japanese", …); empty = detected. */
  alphabet: string;
  /** The game (trading cards) or sport of every card, as Ximilar's taxonomy spells it; empty = detected. */
  subcategory: string;
  scan: ScanSettings;
  syncRemote: boolean;
  engine: Engine;
  localModel: string;
}

export interface SessionCounts {
  total: number;
  done: number;
  errors: number;
  queued: number;
  inProgress: number;
  grading: number;
  synced: number;
  totalPrice: number;
  ebayDrafts: number;
  ebayListed: number;
  ebaySold: number;
  /** Identified cards nobody has accepted or corrected yet. */
  toReview: number;
  /** Paid Ximilar API calls the cards took. */
  calls: number;
}

export interface Session {
  id: string;
  name: string;
  sourceKind: SourceKind;
  sourceRef: string;
  idMode: IdMode;
  options: SessionOptions;
  status: SessionStatus;
  error: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  remoteSessionId: string | null;
  syncError: string | null;
  /** Why the session's last eBay job stopped. */
  ebayError: string | null;
  ebayCheckedAt: string | null;
  counts: SessionCounts;
}

export interface NewSession {
  name: string;
  sourceKind: SourceKind;
  sourceRef: string;
  idMode: IdMode;
  options: SessionOptions;
}

export interface CardFields {
  name: string;
  fullName: string;
  setName: string;
  setCode: string;
  cardNumber: string;
  series: string;
  year: string;
  subcategory: string;
}

export interface SlabLabel {
  company: string;
  grade: string;
  certificate: string;
}

/** Where a card's eBay listing is: an unpublished offer, live, sold, or ended without a sale. */
export type EbayStatus = "draft" | "listed" | "sold" | "ended";

export interface EbayListing {
  /** null: nothing is on eBay yet (the first attempt failed, see `error`). */
  status: EbayStatus | null;
  price: number | null;
  listingId: string | null;
  error: string | null;
}

export interface ItemSummary extends CardFields {
  id: number;
  sessionId: string;
  seq: number;
  status: ItemStatus;
  frontPath: string;
  backPath: string | null;
  thumbPath: string | null;
  idType: string | null;
  confidence: string | null;
  distance: number | null;
  price: number | null;
  grade: number | null;
  condition: string | null;
  /** What the graded slab's label says, when the card is in one. */
  slab: SlabLabel | null;
  error: string | null;
  synced: boolean;
  /** The user picked the match from the candidates. */
  picked: boolean;
  /** The user typed the card's fields or price. */
  edited: boolean;
  /** The user accepted the match (or corrected it). */
  reviewed: boolean;
  /** An AI agent (over MCP) accepted, picked or corrected it, not the user. */
  agentReviewed: boolean;
  /** What the agent said about the card when it did. */
  reviewNote: string;
  notes: string;
  /** Where the card is kept: a box, a binder page. */
  location: string;
  updatedAt: string;
  /** The card's eBay listing, once one was attempted. */
  ebay: EbayListing | null;
}

/** A card on the Cards page: an identified card of any session. */
export interface CardRow extends ItemSummary {
  sessionName: string;
  /** Identified cards with the same name, set and number, this one included. */
  copies: number;
}

export type CardFilter = "all" | "review" | "graded" | "duplicates" | "agent";

/** The cards matching a search: the first ones, and the totals of all of them. */
export interface CardSearch {
  cards: CardRow[];
  total: number;
  value: number;
}

/** What the user can change on a card by hand. */
export interface ItemEdit extends CardFields {
  price: number | null;
  notes: string;
  location: string;
}

/* eslint-disable @typescript-eslint/no-explicit-any */
export type Json = any;

export interface ItemDetail extends ItemSummary {
  eventId: string;
  previewPath: string | null;
  backPreviewPath: string | null;
  recognition: Json | null;
  grading: Json | null;
  gradingImages: string[];
  calls: number;
  analyzedAt: string | null;
  remoteId: string | null;
  ebaySku: string | null;
  ebayOfferId: string | null;
  /** The card's pictures uploaded to eBay. */
  ebayImages: string[];
}

export interface Settings {
  apiBaseUrl: string;
  workspace: string | null;
  defaultFolder: string | null;
  defaultScanner: string | null;
  scan: ScanSettings;
  defaultIdMode: IdMode;
  defaultGrading: GradingMode;
  defaultPairing: Pairing;
  /** The game (TCG) new sessions start with; empty = any. */
  defaultSubcategory: string;
  /** Condition scale of new sessions: tcgplayer, cardmarket, ebay_tcg, ebay_sport (ximilar: the API only). */
  defaultConditionMode: string;
  /** How cards are brought in, the main one first: what "New session" opens. */
  defaultSources: SourceKind[];
  /** The first-run setup was finished or skipped. */
  setupDone: boolean;
  /** The app's language (i18n/languages.ts); anything unknown reads as English. */
  language: string;
  priceStats: boolean;
  concurrency: number;
  syncByDefault: boolean;
  engine: Engine;
  localModel: string;
  ebay: EbaySettings;
  mcp: McpSettings;
}

/** AI agents driving the app over MCP (on this computer only) and what they may do; reading is always allowed. */
export interface McpSettings {
  enabled: boolean;
  port: number;
  /** Create, run and stop sessions; accept, pick, edit, retry and turn cards; sync; CSV exports. */
  allowRun: boolean;
  /** Delete cards and sessions (also in Cards Cloud). */
  allowDelete: boolean;
  /** eBay drafts and listings, Whatnot and Shopify exports. */
  allowMarket: boolean;
  /** Change the defaults of new sessions. */
  allowSettings: boolean;
}

export interface McpStatus {
  enabled: boolean;
  running: boolean;
  /** The server's address, while it runs. */
  url: string | null;
  /** What agents send as `Authorization: Bearer …`. */
  token: string | null;
  /** This app's program: clients that start a program run it with the argument `mcp`. */
  command: string;
  error: string | null;
  /** When an agent last called a tool. */
  lastUsed: string | null;
}

/** The user's eBay developer keyset (the Cert ID is kept apart, like the API token) and what listings use. */
export interface EbaySettings {
  sandbox: boolean;
  appId: string;
  ruName: string;
  apiBase?: string;
  marketplace: string;
  fulfillmentPolicyId: string;
  paymentPolicyId: string;
  returnPolicyId: string;
  locationKey: string;
  /** The connected seller; set when connecting. */
  username: string | null;
}

export interface Named {
  id: string;
  name: string;
}

/** What the connected eBay account offers for listings. */
export interface EbaySetup {
  username: string | null;
  /** Business policies are turned on; listings need them. */
  optedIn: boolean;
  fulfillmentPolicies: Named[];
  paymentPolicies: Named[];
  returnPolicies: Named[];
  locations: Named[];
}

/** One card in the "List on eBay" dialog. */
export interface EbayPreview {
  itemId: number;
  seq: number;
  thumbPath: string | null;
  title: string;
  category: string | null;
  condition: string | null;
  /** The price it is offered at, else Ximilar's price. */
  price: number | null;
  ebay: EbayListing | null;
  /** Why it can't be listed. */
  problem: string | null;
}

/** The condition of ungraded cards. */
export type CardCondition = "near_mint" | "excellent" | "very_good" | "poor";

export interface EbayProgress {
  sessionId: string;
  action: "drafts" | "publish" | "refresh" | "remove" | "";
  done: number;
  total: number;
  running: boolean;
}

/** A marketplace file: TCGplayer's list of ungraded cards, or Whatnot's bulk listing CSV. */
export type ExportFormat = "tcgplayer" | "whatnot" | "shopify";

/** A card left out of an export, and why. */
export interface Skipped {
  seq: number;
  reason: string;
}

export interface TcgplayerExport {
  rows: number;
  skipped: Skipped[];
}

export interface WhatnotOptions {
  /** Auction: the price is the starting bid. */
  listingType: "auction" | "buy_it_now";
  shippingProfile: string;
  offerable: boolean;
  /** For ungraded cards whose condition wasn't graded. */
  condition: CardCondition;
  uploadImages: boolean;
}

export interface WhatnotExport {
  rows: number;
  skipped: Skipped[];
  withoutImages: number;
  /** Why pictures stopped being uploaded. */
  imageError: string | null;
}

export interface ShopifyOptions {
  /** Products: new products with variants; inventory: stock of products imported before. */
  file: "products" | "inventory";
  /** by_card: one product per card, its conditions as variants; per_card: one product per scanned card. */
  grouping: "by_card" | "per_card";
  condition: CardCondition;
  /** Active (published) products instead of drafts. */
  active: boolean;
  uploadImages: boolean;
  /** The store location for the inventory file, exactly as Shopify names it. */
  location: string;
}

export interface ShopifyExport {
  products: number;
  variants: number;
  skipped: Skipped[];
  withoutImages: number;
  imageError: string | null;
}

export interface ExportProgress {
  sessionId: string;
  done: number;
  total: number;
}

/** An on-device model as the Settings page shows it. */
export interface ModelStatus {
  id: string;
  name: string;
  /** "Fast", "Balanced", "Accurate". */
  tier: string;
  /** Parameter count, e.g. "450M". */
  params: string;
  description: string;
  /** Memory the loaded model needs, roughly. */
  memoryMb: number;
  licenseName: string;
  licenseUrl: string;
  homepage: string;
  /** Bytes, both files. */
  size: number;
  downloaded: number;
  state: "missing" | "downloading" | "ready";
  error: string | null;
  /** This build can run local models. */
  supported: boolean;
}

export interface CompareRow {
  itemId: number;
  seq: number;
  ximilar: string;
  local: string;
  nameMatches: boolean;
  numberMatches: boolean;
  seconds: number;
  error: string | null;
}

/** How often the local model agrees with what Ximilar identified. */
export interface CompareReport {
  model: string;
  cards: number;
  names: number;
  numbers: number;
  withNumber: number;
  secondsPerCard: number;
  cancelled: boolean;
  rows: CompareRow[];
}

/** Cards analysed on one local day (`YYYY-MM-DD`). */
export interface DayCount {
  date: string;
  cards: number;
  /** Paid Ximilar API calls of the cards analysed that day. */
  calls: number;
}

export interface SettingsView extends Settings {
  tokenSet: boolean;
  /** "dev": a development build on macOS, which keeps the token in the settings file. */
  tokenStorage: "keychain" | "file" | "dev" | null;
  ebayCertSet: boolean;
  ebayConnected: boolean;
  /** The ImgBB key that hosts pictures for the Whatnot export. */
  imgbbKeySet: boolean;
}

export interface AppInfo {
  version: string;
  /** Where sessions, thumbnails and the database are kept. */
  dataDir: string;
  dataBytes: number;
}

export interface ConnectionCheck {
  ok: boolean;
  account: string | null;
  error: string | null;
  cardsCloud: boolean;
  cardsCloudError: string | null;
}

export interface DeviceInfo {
  id: string;
  name: string;
  backend: string;
}

export interface ScannerSupport {
  backend: "sane" | "twain";
  available: boolean;
  hint: string | null;
}

export interface Page<T> {
  count: number;
  next: string | null;
  previous: string | null;
  results: T[];
}

/** A session in Cards Cloud as the API returns it (snake_case). */
export interface RemoteSession {
  id: string;
  name: string;
  description: string;
  game: string;
  platform: string;
  status: "live" | "closed";
  started: string;
  ended: string | null;
  identification_count: number;
  total_price: number;
  client: Json;
  meta_data: Json;
}

/** A Cards Cloud row (snake_case). */
export interface RemoteItem {
  id: string;
  event_id: string;
  seen: string;
  id_type: string;
  name: string;
  full_name: string;
  set_name: string;
  set_code: string;
  card_number: string;
  series: string;
  year: string;
  subcategory: string;
  distance: number | null;
  confidence: string | null;
  links: Record<string, string>;
  alternatives: Json[];
  price_stats: Json[];
  price: number | null;
  image: string | null;
  thumbnail: string | null;
  grading?: Json;
  grade?: number | null;
  sold: boolean;
  note: string;
}

export const api = {
  getSettings: () => invoke<SettingsView>("get_settings"),
  saveSettings: (settings: Settings) => invoke<SettingsView>("save_settings", { settings }),
  setToken: (token: string) => invoke<SettingsView>("set_token", { token }),
  testConnection: () => invoke<ConnectionCheck>("test_connection"),
  mcpStatus: () => invoke<McpStatus>("mcp_status"),
  /** A new token for agents; the old one stops working. */
  regenerateMcpToken: () => invoke<McpStatus>("regenerate_mcp_token"),

  listSessions: () => invoke<Session[]>("list_sessions"),
  dashboardActivity: (days: number) => invoke<DayCount[]>("dashboard_activity", { days }),
  getSession: (id: string) => invoke<Session>("get_session", { id }),
  /** `remoteSession`: the ID or link of a Cards Cloud session to continue; files whose cards it has are skipped. */
  createSession: (session: NewSession, remoteSession?: string) =>
    invoke<Session>("create_session", { session, remoteSession: remoteSession || null }),
  /** Settings apply to the cards analysed from then on; not while the session runs. */
  updateSession: (id: string, name: string, idMode: IdMode, options: SessionOptions) =>
    invoke<Session>("update_session", { id, name, idMode, options }),
  renameSession: (id: string, name: string) => invoke<Session>("rename_session", { id, name }),
  deleteSession: (id: string, deleteRemote = false) => invoke<void>("delete_session", { id, deleteRemote }),
  /** The session's image folder (the scans, for scanner sessions). */
  sessionFolder: (id: string) => invoke<string>("session_folder", { id }),
  exportCsv: (id: string, path: string) => invoke<number>("export_csv", { id, path }),
  previewCsv: (path: string) => invoke<CsvPreview>("preview_csv", { path }),
  /** A new session from a CSV; with `reanalyze`, identified rows with an image are analysed again. */
  importCsv: (session: NewSession, reanalyze: boolean) => invoke<Session>("import_csv", { session, reanalyze }),
  /** `source: false` analyses only the queued cards (cards to analyse again): no new scan, no new files. */
  runSession: (id: string, source = true) => invoke<void>("run_session", { id, source }),
  pauseSession: (id: string) => invoke<void>("pause_session", { id }),
  stopSession: (id: string) => invoke<void>("stop_session", { id }),
  retryItems: (id: string, itemIds?: number[]) => invoke<number>("retry_items", { id, itemIds }),
  syncSession: (id: string) => invoke<void>("sync_session", { id }),
  listItems: (sessionId: string) => invoke<ItemSummary[]>("list_items", { sessionId }),
  getItem: (id: number) => invoke<ItemDetail>("get_item", { id }),
  /** Makes candidate `index` (into the response's alternatives) the card's match. */
  selectMatch: (id: number, index: number) => invoke<ItemDetail>("select_match", { id, index }),
  /** Identified cards of every session, newest first, searched by name, set, number, location and notes. */
  searchCards: (query: string, filter: CardFilter, limit?: number) =>
    invoke<CardSearch>("search_cards", { query, filter, limit }),
  /** Saves what the user typed: the card fields and price (once identified), notes and location. */
  updateItem: (id: number, edit: ItemEdit) => invoke<ItemDetail>("update_item", { id, edit }),
  /** Turns a card's image upright when Ximilar found it upside down (cardloader's copies only). */
  turnItem: (id: number) => invoke<ItemDetail>("turn_item", { id }),
  /** Turns upright those of the cards Ximilar found upside down; the others are counted as skipped. */
  turnItems: (itemIds: number[]) => invoke<{ turned: number; skipped: number }>("turn_items", { itemIds }),
  /** Accepts the cards' matches, or with `reviewed: false` takes that back. */
  reviewItems: (sessionId: string, itemIds: number[], reviewed = true) =>
    invoke<number>("review_items", { sessionId, itemIds, reviewed }),
  /** Sets where the cards are kept; empty clears it. */
  locateItems: (sessionId: string, itemIds: number[], location: string) =>
    invoke<number>("locate_items", { sessionId, itemIds, location }),
  /** Deletes cards from a stopped session; running the folder again doesn't bring them back. */
  deleteItems: (sessionId: string, itemIds: number[]) => invoke<number>("delete_items", { sessionId, itemIds }),
  countImages: (path: string) => invoke<number>("count_images", { path }),
  appInfo: () => invoke<AppInfo>("app_info"),

  listModels: () => invoke<ModelStatus[]>("list_models"),
  /** Starts or resumes a download; progress arrives through onModelUpdated. */
  downloadModel: (id: string) => invoke<ModelStatus>("download_model", { id }),
  cancelModelDownload: (id: string) => invoke<void>("cancel_model_download", { id }),
  deleteModel: (id: string) => invoke<ModelStatus>("delete_model", { id }),
  /** `runId` (chosen by the caller) names the comparison for its progress events and for cancelling it. */
  compareLocal: (sessionId: string, runId: string, model?: string) =>
    invoke<CompareReport>("compare_local", { sessionId, runId, model }),
  cancelCompare: (runId: string) => invoke<void>("cancel_compare", { runId }),

  scannerSupport: () => invoke<ScannerSupport>("scanner_support"),
  listScanners: () => invoke<DeviceInfo[]>("list_scanners"),
  /** Opens the TWAIN driver's settings dialog; true when its settings were saved for later scans. */
  scannerDriverSettings: (device: string) => invoke<boolean>("scanner_driver_settings", { device }),

  listRemoteSessions: (page: number, search = "") =>
    invoke<Page<RemoteSession>>("list_remote_sessions", { page, search }),
  renameRemoteSession: (id: string, name: string) => invoke<RemoteSession>("rename_remote_session", { id, name }),
  deleteRemoteSession: (id: string) => invoke<void>("delete_remote_session", { id }),
  getRemoteSession: (id: string) => invoke<{ session: RemoteSession; summary: Json }>("get_remote_session", { id }),
  listRemoteItems: (sessionId: string, page: number) =>
    invoke<Page<RemoteItem>>("list_remote_items", { sessionId, page }),

  setEbayCert: (certId: string) => invoke<SettingsView>("set_ebay_cert", { certId }),
  /** Opens eBay's consent page in a window and keeps the login. */
  ebayConnect: () => invoke<SettingsView>("ebay_connect"),
  /** Connects with the address (or code) eBay showed after consent. */
  ebayConnectWith: (text: string) => invoke<SettingsView>("ebay_connect_with", { text }),
  ebayDisconnect: () => invoke<SettingsView>("ebay_disconnect"),
  ebaySetup: () => invoke<EbaySetup>("ebay_setup"),
  ebayOptIn: () => invoke<EbaySetup>("ebay_opt_in"),
  ebayCreateLocation: (postalCode: string, country: string) =>
    invoke<EbaySetup>("ebay_create_location", { postalCode, country }),
  ebayPreview: (sessionId: string) => invoke<EbayPreview[]>("ebay_preview", { sessionId }),
  /** Unpublished offers (drafts) at these prices; progress arrives through onEbayProgress. */
  ebayCreateDrafts: (sessionId: string, items: { itemId: number; price: number }[], condition: CardCondition) =>
    invoke<void>("ebay_create_drafts", { sessionId, items, condition }),
  /** Publishes the session's drafts (or the given cards'). */
  ebayPublish: (sessionId: string, itemIds?: number[]) => invoke<void>("ebay_publish", { sessionId, itemIds }),
  ebayRefresh: (sessionId: string, force = false) => invoke<void>("ebay_refresh", { sessionId, force }),
  /** Deletes drafts and ends live listings. */
  ebayRemove: (sessionId: string, itemIds: number[]) => invoke<void>("ebay_remove", { sessionId, itemIds }),
  ebayCancel: (sessionId: string) => invoke<void>("ebay_cancel", { sessionId }),
  ebayJob: (sessionId: string) => invoke<EbayProgress | null>("ebay_job", { sessionId }),
  ebayListingUrl: (listingId: string) => invoke<string>("ebay_listing_url", { listingId }),

  setImgbbKey: (key: string) => invoke<SettingsView>("set_imgbb_key", { key }),
  /** The session's identified cards as the marketplace would get them. */
  exportPreview: (sessionId: string, format: ExportFormat) =>
    invoke<EbayPreview[]>("export_preview", { sessionId, format }),
  exportTcgplayer: (
    sessionId: string,
    items: { itemId: number; price: number }[],
    condition: CardCondition,
    path: string,
  ) => invoke<TcgplayerExport>("export_tcgplayer", { sessionId, items, condition, path }),
  /** Uploads the pictures (progress through onExportProgress), then writes the file. */
  exportWhatnot: (
    sessionId: string,
    items: { itemId: number; price: number }[],
    options: WhatnotOptions,
    path: string,
  ) => invoke<WhatnotExport>("export_whatnot", { sessionId, items, options, path }),
  /** Product CSV (pictures uploaded first; progress through onExportProgress) or inventory CSV. */
  exportShopify: (
    sessionId: string,
    items: { itemId: number; price: number }[],
    options: ShopifyOptions,
    path: string,
  ) => invoke<ShopifyExport>("export_shopify", { sessionId, items, options, path }),
  cancelExport: (sessionId: string) => invoke<void>("cancel_export", { sessionId }),
};

export function onSessionUpdated(cb: (s: Session) => void): Promise<UnlistenFn> {
  return listen<Session>("session-updated", (e) => cb(e.payload));
}

/** A session was deleted (by an agent, or elsewhere in the app). */
export function onSessionDeleted(cb: (id: string) => void): Promise<UnlistenFn> {
  return listen<string>("session-deleted", (e) => cb(e.payload));
}

export function onModelUpdated(cb: (m: ModelStatus) => void): Promise<UnlistenFn> {
  return listen<ModelStatus>("model-updated", (e) => cb(e.payload));
}

export interface CompareProgress {
  runId: string;
  done: number;
  total: number;
  row: CompareRow;
}

export function onCompareProgress(cb: (p: CompareProgress) => void): Promise<UnlistenFn> {
  return listen<CompareProgress>("compare-progress", (e) => cb(e.payload));
}

export function onEbayProgress(cb: (p: EbayProgress) => void): Promise<UnlistenFn> {
  return listen<EbayProgress>("ebay-progress", (e) => cb(e.payload));
}

export function onExportProgress(cb: (p: ExportProgress) => void): Promise<UnlistenFn> {
  return listen<ExportProgress>("export-progress", (e) => cb(e.payload));
}

export function onItemUpdated(cb: (i: ItemSummary) => void): Promise<UnlistenFn> {
  return listen<ItemSummary>("item-updated", (e) => cb(e.payload));
}

/** Errors from commands arrive as strings. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
