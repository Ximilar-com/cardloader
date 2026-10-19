// One shape for the table and detail panel, built from a local item or a Cards Cloud row.

import type { ItemDetail, ItemStatus, ItemSummary, Json, RemoteItem, SlabLabel } from "./api";
import { fileUrl } from "./format";

export interface Row {
  key: string;
  seq: number;
  thumb?: string;
  /** The card's own name; the set and number have their own column. */
  name: string;
  fullName: string;
  setName: string;
  cardNumber: string;
  idType: string | null;
  confidence: string | null;
  price: number | null;
  grade: number | null;
  condition: string | null;
  slab?: SlabLabel | null;
  status?: ItemStatus;
  error?: string | null;
  synced?: boolean;
  picked?: boolean;
}

export interface CardImage {
  label: string;
  src?: string;
  /** Local file to open in the system viewer. */
  path?: string;
}

export interface CardView {
  title: string;
  /** The user picked the match. */
  picked?: boolean;
  status?: ItemStatus;
  error?: string | null;
  fields: [string, string][];
  idType: string | null;
  confidence: string | null;
  distance: number | null;
  price: number | null;
  priceStats: Json[];
  links: Record<string, string>;
  alternatives: Candidate[];
  slab: Json | null;
  grade: number | null;
  condition: string | null;
  gradingEndpoint: string | null;
  gradingRecords: Json[];
  images: CardImage[];
  gradingImages: CardImage[];
  raw: Json;
  /** Read by an on-device model: which one, and how long it took. */
  local?: { model: string; seconds: number | null };
  /** How Ximilar read it, when not the usual way: "front + back", "Magic AI", a custom endpoint. */
  via?: string;
}

/** Another possible match, after the best one. */
export interface Candidate {
  /** Index into the response's `alternatives`. */
  index: number;
  name: string;
  /** "Set (CODE) · #12 · 2000" */
  detail: string;
  fullName: string;
  distance: number | null;
  confidence: string | null;
  price: number | null;
}

/** Same tiers as the Rust side (`recognition::tier`). */
export function tier(distance: number | null | undefined): string | null {
  if (distance == null || !Number.isFinite(distance)) return null;
  return distance <= 0.3 ? "high" : distance <= 0.4 ? "medium" : "low";
}

/** The ungraded median, else the graded one, else overall (same rule as the Rust side). */
export function representativePrice(stats: Json[] | null | undefined): number | null {
  for (const kind of ["ungraded", "graded", "overall"]) {
    for (const e of stats ?? []) {
      if (e?.stats_type !== kind) continue;
      const median = (e.value ?? e)?.median;
      if (typeof median === "number" && Number.isFinite(median) && median >= 0) return median;
    }
  }
  return null;
}

const str = (v: unknown): string => (v == null ? "" : String(v).trim());

/** `distances[0]` belongs to the best match, so alternative `i` has `distances[i + 1]`. */
export function candidates(alternatives: Json[] | null | undefined, distances?: Json[] | null): Candidate[] {
  const list: Candidate[] = [];
  (alternatives ?? []).forEach((a: Json, i: number) => {
    if (!a || typeof a !== "object") return;
    const set = str(a.set ?? a.set_name ?? a.publisher);
    const code = str(a.set_code);
    const number = str(a.card_number ?? a.card_no ?? a.number);
    const parts = [code && set ? `${set} (${code})` : set || code, number && `#${number}`, str(a.year ?? a.date)];
    const fullName = str(a.full_name);
    const raw = a.distance ?? distances?.[i + 1];
    const distance = typeof raw === "number" ? raw : null;
    list.push({
      index: i,
      name: str(a.name ?? a.title) || fullName || "Unknown",
      detail: parts.filter(Boolean).join(" · "),
      fullName,
      distance,
      confidence: tier(distance),
      price: representativePrice(a.price_stats),
    });
  });
  return list;
}

export function rowFromItem(i: ItemSummary): Row {
  return {
    key: String(i.id),
    seq: i.seq,
    thumb: fileUrl(i.thumbPath),
    name: i.name || i.fullName,
    fullName: i.fullName || i.name,
    setName: i.setName,
    cardNumber: i.cardNumber,
    idType: i.idType,
    confidence: i.confidence,
    price: i.price,
    grade: i.grade,
    condition: i.condition,
    slab: i.slab,
    status: i.status,
    error: i.error,
    synced: i.synced,
    picked: i.picked,
  };
}

export function rowFromRemote(r: RemoteItem, seq: number): Row {
  return {
    key: r.id,
    seq,
    thumb: r.thumbnail ?? undefined,
    name: r.name || r.full_name,
    fullName: r.full_name || r.name,
    setName: r.set_name,
    cardNumber: r.card_number,
    idType: r.id_type,
    confidence: r.confidence,
    price: r.price,
    grade: r.grade ?? null,
    condition: r.grading?.grades?.condition ?? null,
  };
}

function area(o: Json): number {
  if (typeof o?.area === "number") return o.area;
  const b = o?.bound_box;
  return Array.isArray(b) && b.length === 4 ? Math.abs((b[2] - b[0]) * (b[3] - b[1])) : 0;
}

/** The identification of the main object of an id-endpoint response, and the slab label's, if any. */
/** One record's main object: the largest identified object that isn't a slab label, else any identified one. */
function recordIdent(record: Json): Json | null {
  const objects: Json[] = record?._objects ?? [];
  const sorted = [...objects].sort((a, b) => area(b) - area(a));
  const main =
    sorted.find((o) => o?.name !== "Slab Label" && o?._identification) ??
    objects.find((o) => o?._identification) ??
    null;
  return main?._identification ?? record?._identification ?? null;
}

export function matchOf(response: Json): { ident: Json | null; slab: Json | null } {
  // Same choice as the Rust side (`recognition::ident_pointer`): when the back was sent too, the side whose
  // best match is closest; the front on a tie.
  const distance = (ident: Json | null) =>
    ident?.best_match && typeof ident.best_match === "object"
      ? typeof ident.distances?.[0] === "number"
        ? ident.distances[0]
        : Number.MAX_VALUE
      : Infinity;
  let ident: Json | null = null;
  for (const record of response?.records ?? []) {
    const candidate = recordIdent(record);
    if (candidate && (ident === null || distance(candidate) < distance(ident))) ident = candidate;
  }
  const objects: Json[] = response?.records?.[0]?._objects ?? [];
  const slabObj = objects.find((o) => o?.name === "Slab Label") ?? null;
  const slab = slabObj
    ? {
        ...(slabObj._identification?.best_match ?? {}),
        company: slabObj._tags?.Company?.[0]?.name || slabObj._identification?.best_match?.company,
        tagGrade: slabObj._tags?.Grade?.[0]?.name,
      }
    : null;
  return { ident, slab };
}

const FIELD_LABELS: [keyof CardFieldsLike, string][] = [
  ["setName", "Set"],
  ["setCode", "Set code"],
  ["cardNumber", "Number"],
  ["year", "Year"],
  ["series", "Series"],
  ["subcategory", "Category"],
];

interface CardFieldsLike {
  setName: string;
  setCode: string;
  cardNumber: string;
  year: string;
  series: string;
  subcategory: string;
}

function fields(c: CardFieldsLike): [string, string][] {
  return FIELD_LABELS.map(([k, label]) => [label, c[k]] as [string, string]).filter(([, v]) => !!v);
}

export function viewFromItem(i: ItemDetail): CardView {
  const { ident, slab } = matchOf(i.recognition?.response);
  const best = ident?.best_match ?? {};
  const images: CardImage[] = [{ label: "Front", src: fileUrl(i.previewPath), path: i.frontPath }];
  if (i.backPath) images.push({ label: "Back", src: fileUrl(i.backPreviewPath), path: i.backPath });
  return {
    title: i.fullName || i.name || (i.status === "done" ? "No match" : "—"),
    picked: i.picked,
    status: i.status,
    error: i.error,
    fields: fields(i),
    idType: i.idType,
    confidence: i.confidence,
    distance: i.distance,
    price: i.price,
    priceStats: best.price_stats ?? [],
    links: best.links ?? {},
    alternatives: candidates(ident?.alternatives, ident?.distances),
    slab,
    grade: i.grade,
    condition: i.condition,
    gradingEndpoint: i.grading?.endpoint ?? null,
    gradingRecords: i.grading?.response?.records ?? [],
    images,
    gradingImages: i.gradingImages.map((p) => ({ label: labelOf(p), src: fileUrl(p), path: p })),
    raw: { recognition: i.recognition, grading: i.grading },
    via: viaOf(i.recognition),
    local: String(i.recognition?.endpoint ?? "").startsWith("local/")
      ? { model: String(i.recognition.model ?? ""), seconds: i.recognition.seconds ?? null }
      : undefined,
  };
}

const STANDARD_ENDPOINT = /^collectibles\/v2\/((tcg|sport|comics|slab)_id|analyze)$/;

function viaOf(recognition: Json): string | undefined {
  if (!recognition) return undefined;
  const endpoint = String(recognition.endpoint ?? "");
  const parts = [
    recognition.sides?.length > 1 && "front + back",
    recognition.magic_ai_used && "Magic AI",
    recognition.skip_detection && "whole image",
    recognition.id_type && !STANDARD_ENDPOINT.test(endpoint) && endpoint,
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : undefined;
}

function labelOf(path: string): string {
  if (path.includes("exact")) return "Grading overlay";
  if (path.includes("clean")) return "Cropped card";
  if (path.includes("full")) return "Full overlay";
  return "Result";
}

export function viewFromRemote(r: RemoteItem): CardView {
  return {
    title: r.full_name || r.name || "No match",
    fields: fields({
      setName: r.set_name,
      setCode: r.set_code,
      cardNumber: r.card_number,
      year: r.year,
      series: r.series,
      subcategory: r.subcategory,
    }),
    idType: r.id_type,
    confidence: r.confidence,
    distance: r.distance,
    price: r.price,
    priceStats: r.price_stats ?? [],
    links: r.links ?? {},
    alternatives: candidates(r.alternatives),
    slab: null,
    grade: r.grade ?? null,
    condition: r.grading?.grades?.condition ?? null,
    gradingEndpoint: r.grading?.endpoint ?? null,
    gradingRecords: r.grading?.records ?? [],
    images: r.image ? [{ label: "Card", src: r.image }] : [],
    gradingImages: [],
    raw: r,
  };
}
