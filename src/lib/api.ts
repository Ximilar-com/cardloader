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

export interface ScanSettings {
  dpi: number;
  color: ColorMode;
  duplex: boolean;
  pageWidthMm: number;
  pageHeightMm: number;
  multifeed: MultifeedMode;
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
  updatedAt: string;
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
  priceStats: boolean;
  concurrency: number;
  syncByDefault: boolean;
  engine: Engine;
  localModel: string;
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

export interface SettingsView extends Settings {
  tokenSet: boolean;
  /** "dev": a development build on macOS, which keeps the token in the settings file. */
  tokenStorage: "keychain" | "file" | "dev" | null;
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

  listSessions: () => invoke<Session[]>("list_sessions"),
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
  runSession: (id: string) => invoke<void>("run_session", { id }),
  pauseSession: (id: string) => invoke<void>("pause_session", { id }),
  stopSession: (id: string) => invoke<void>("stop_session", { id }),
  retryItems: (id: string, itemIds?: number[]) => invoke<number>("retry_items", { id, itemIds }),
  syncSession: (id: string) => invoke<void>("sync_session", { id }),
  listItems: (sessionId: string) => invoke<ItemSummary[]>("list_items", { sessionId }),
  getItem: (id: number) => invoke<ItemDetail>("get_item", { id }),
  /** Makes candidate `index` (into the response's alternatives) the card's match. */
  selectMatch: (id: number, index: number) => invoke<ItemDetail>("select_match", { id, index }),
  countImages: (path: string) => invoke<number>("count_images", { path }),

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

  listRemoteSessions: (page: number, search = "") =>
    invoke<Page<RemoteSession>>("list_remote_sessions", { page, search }),
  renameRemoteSession: (id: string, name: string) => invoke<RemoteSession>("rename_remote_session", { id, name }),
  deleteRemoteSession: (id: string) => invoke<void>("delete_remote_session", { id }),
  getRemoteSession: (id: string) => invoke<{ session: RemoteSession; summary: Json }>("get_remote_session", { id }),
  listRemoteItems: (sessionId: string, page: number) =>
    invoke<Page<RemoteItem>>("list_remote_items", { sessionId, page }),
};

export function onSessionUpdated(cb: (s: Session) => void): Promise<UnlistenFn> {
  return listen<Session>("session-updated", (e) => cb(e.payload));
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

export function onItemUpdated(cb: (i: ItemSummary) => void): Promise<UnlistenFn> {
  return listen<ItemSummary>("item-updated", (e) => cb(e.payload));
}

/** Errors from commands arrive as strings. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
