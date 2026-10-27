// Browser preview: `npm run dev` outside Tauri answers the Rust commands with demo data, so the UI can be
// developed (and reviewed) in a plain browser. Never part of a production build.

import { emit } from "@tauri-apps/api/event";
import { mockConvertFileSrc, mockIPC } from "@tauri-apps/api/mocks";
import type {
  EbayListing,
  EbayPreview,
  EbayProgress,
  ItemDetail,
  ItemEdit,
  ItemSummary,
  Json,
  ModelStatus,
  Session,
  SettingsView,
} from "./api";
import { matchOf, needsReview, representativePrice, tier } from "./cardview";

const CARDS = [
  ["Charizard ex", "Obsidian Flames", "OBF", "125", "2023", "Pokemon", 0.12, 24.99, 9.0, "Mint", "#f97316"],
  ["Black Lotus", "Alpha", "LEA", "232", "1993", "Magic The Gathering", 0.22, 18500, null, null, "#334155"],
  [
    "Blue-Eyes White Dragon",
    "Legend of Blue Eyes",
    "LOB",
    "001",
    "2002",
    "Yu-Gi-Oh!",
    0.35,
    89.5,
    7.5,
    "Near Mint",
    "#2563eb",
  ],
  ["Monkey D. Luffy", "Romance Dawn", "OP01", "024", "2022", "One Piece", 0.18, 12.0, 8.5, "Near Mint", "#dc2626"],
  ["Shohei Ohtani", "Topps Chrome", "TC", "150", "2018", "Baseball", 0.28, 310.0, 6.0, "Excellent", "#0d9488"],
  ["Pikachu", "Base Set", "BS", "58", "1999", "Pokemon", 0.44, 4.5, 8.0, "Near Mint", "#eab308"],
] as const;

function cardSvg(name: string, color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="315" height="440" viewBox="0 0 315 440">
    <rect width="315" height="440" rx="16" fill="${color}"/>
    <rect x="18" y="18" width="279" height="404" rx="10" fill="white" opacity="0.92"/>
    <rect x="34" y="70" width="247" height="190" rx="6" fill="${color}" opacity="0.35"/>
    <text x="34" y="54" font-family="sans-serif" font-size="20" font-weight="700" fill="#0f172a">${name}</text>
    <rect x="34" y="285" width="247" height="10" rx="5" fill="#cbd5e1"/>
    <rect x="34" y="305" width="200" height="10" rx="5" fill="#cbd5e1"/>
    <rect x="34" y="325" width="220" height="10" rx="5" fill="#cbd5e1"/></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

const now = () => new Date().toISOString();

const settings: SettingsView = {
  apiBaseUrl: "https://api.ximilar.com",
  workspace: null,
  defaultFolder: "/Users/demo/Cards",
  defaultScanner: "fujitsu:fi-8170:1234",
  scan: {
    dpi: 600,
    color: "color",
    duplex: true,
    pageWidthMm: 66,
    pageHeightMm: 92,
    multifeed: "off",
    rotation: "none",
    backFirst: false,
  },
  defaultIdMode: "auto",
  defaultGrading: "grade",
  defaultPairing: "single",
  defaultSubcategory: "",
  defaultConditionMode: "ebay_tcg",
  defaultSources: ["scanner", "folder"],
  // The demo starts with the setup, as a first launch does.
  setupDone: false,
  priceStats: true,
  concurrency: 3,
  syncByDefault: true,
  engine: "ximilar",
  localModel: "lfm2.5-vl-3b",
  ebay: {
    sandbox: true,
    appId: "Demo-Cardload-SBX-0a1b2c3d4-5e6f7a8b",
    ruName: "Demo-Demo-Cardload-abcdef",
    marketplace: "EBAY_US",
    fulfillmentPolicyId: "ship-1",
    paymentPolicyId: "pay-1",
    returnPolicyId: "return-1",
    locationKey: "home",
    username: "demo-seller",
  },
  tokenSet: true,
  tokenStorage: "keychain",
  ebayCertSet: true,
  ebayConnected: true,
  imgbbKeySet: true,
};

const ebaySetup = {
  username: "demo-seller",
  optedIn: true,
  fulfillmentPolicies: [
    { id: "ship-1", name: "Cards — tracked letter" },
    { id: "ship-2", name: "Cards — free shipping over $50" },
  ],
  paymentPolicies: [{ id: "pay-1", name: "Managed payments" }],
  returnPolicies: [
    { id: "return-1", name: "30 days, buyer pays" },
    { id: "return-2", name: "No returns" },
  ],
  locations: [{ id: "home", name: "Cardloader (Portland 97201 US)" }],
};

const sessions: Session[] = [];
const items = new Map<string, ItemDetail[]>();

function counts(id: string) {
  const list = items.get(id) ?? [];
  const n = (s: string) => list.filter((i) => i.status === s).length;
  return {
    total: list.length,
    done: n("done"),
    errors: n("error"),
    queued: n("queued"),
    inProgress: n("recognizing"),
    grading: n("grading"),
    synced: list.filter((i) => i.synced).length,
    totalPrice: list.reduce((a, i) => a + (i.price ?? 0), 0),
    ebayDrafts: list.filter((i) => i.ebay?.status === "draft").length,
    ebayListed: list.filter((i) => i.ebay?.status === "listed").length,
    ebaySold: list.filter((i) => i.ebay?.status === "sold").length,
    toReview: list.filter(needsReview).length,
    calls: list.reduce((a, i) => a + i.calls, 0),
  };
}

/** A copy, as the real IPC delivers: React must see a new object. */
function session(id: string): Session {
  const s = sessions.find((x) => x.id === id)!;
  s.counts = counts(id);
  return structuredClone(s);
}

function makeItem(sessionId: string, seq: number): ItemDetail {
  const [name, setName, setCode, cardNumber, year, subcategory, distance, price, grade, condition, color] =
    CARDS[(seq - 1) % CARDS.length];
  const img = cardSvg(name, color);
  // Every third card is in a graded slab.
  const slab =
    seq % 3 === 0
      ? {
          company: ["PSA", "BGS", "CGC"][(seq / 3 - 1) % 3],
          grade: ["10", "9.5", "9"][(seq / 3 - 1) % 3],
          certificate: String(81_204_310 + seq * 7),
        }
      : null;
  return {
    id: Number(`${sessions.findIndex((s) => s.id === sessionId) + 1}${String(seq).padStart(4, "0")}`),
    sessionId,
    seq,
    status: "done",
    frontPath: `/Users/demo/Cards/card_${String(seq).padStart(3, "0")}_front.jpg`,
    backPath: `/Users/demo/Cards/card_${String(seq).padStart(3, "0")}_back.jpg`,
    thumbPath: img,
    previewPath: img,
    backPreviewPath: cardSvg("Back", "#475569"),
    idType: subcategory === "Baseball" ? "sport" : "tcg",
    name,
    fullName: `${name} ${setName} (${setCode}) #${cardNumber}`,
    setName,
    setCode,
    cardNumber,
    series: "",
    year,
    subcategory,
    confidence: distance <= 0.3 ? "high" : distance <= 0.4 ? "medium" : "low",
    distance,
    price,
    grade,
    condition,
    slab,
    error: null,
    synced: true,
    picked: false,
    edited: false,
    reviewed: false,
    notes: "",
    location: seq <= 6 ? "Box 1" : "",
    updatedAt: now(),
    eventId: `demo${seq}`,
    remoteId: null,
    ebay: null,
    ebaySku: null,
    ebayOfferId: null,
    ebayImages: [],
    recognition: {
      endpoint: subcategory === "Baseball" ? "collectibles/v2/sport_id" : "collectibles/v2/tcg_id",
      id_type: subcategory === "Baseball" ? "sport" : "tcg",
      // A sports card read from both sides, with magic AI.
      ...(subcategory === "Baseball" ? { sides: ["front", "back"], magic_ai_used: true } : {}),
      response: {
        records: [
          {
            _objects: [
              {
                name: "Card",
                // Every fourth card was fed upside down.
                ...(seq % 4 === 0
                  ? { _tags: { Rotation: [{ id: "rot", name: "rotated_upside_down", prob: 0.99893 }] } }
                  : {}),
                _identification: {
                  best_match: {
                    name,
                    full_name: `${name} ${setName} (${setCode}) #${cardNumber}`,
                    set: setName,
                    set_code: setCode,
                    card_number: cardNumber,
                    year,
                    links: { "tcgplayer.com": "https://www.tcgplayer.com", "ebay.com": "https://www.ebay.com" },
                    price_stats: [
                      {
                        stats_type: "ungraded",
                        value: { median: price, min: price * 0.7, max: price * 1.6, latest: price * 1.1 },
                      },
                      {
                        stats_type: "graded",
                        value: { median: price * 2.4, min: price * 1.5, max: price * 4, latest: price * 2.2 },
                      },
                    ],
                  },
                  alternatives: [
                    {
                      name,
                      full_name: `${name} ${setName} (${setCode}) #${cardNumber}a`,
                      set: setName,
                      set_code: setCode,
                      card_number: `${cardNumber}a`,
                      year,
                      links: { "tcgplayer.com": "https://www.tcgplayer.com" },
                      price_stats: [{ stats_type: "ungraded", value: { median: price * 1.3 } }],
                    },
                    {
                      name,
                      full_name: `${name} Commander Masters (CMM) #517`,
                      set: "Commander Masters",
                      set_code: "CMM",
                      card_number: "517",
                      year: "2023",
                      links: { "tcgplayer.com": "https://www.tcgplayer.com" },
                      price_stats: [{ stats_type: "ungraded", value: { median: price * 0.4 } }],
                    },
                    {
                      name: "Port Town",
                      full_name: "Port Town Commander Masters (CMM) #1019",
                      set: "Commander Masters",
                      set_code: "CMM",
                      card_number: "1019",
                      year: "2023",
                    },
                    {
                      name: "The Great Henge",
                      full_name: "The Great Henge Commander Masters (CMM) #294",
                      set: "Commander Masters",
                      set_code: "CMM",
                      card_number: "294",
                      year: "2023",
                      price_stats: [{ stats_type: "ungraded", value: { median: 31.2 } }],
                    },
                  ],
                  distances: [distance, distance + 0.04, distance + 0.11, distance + 0.19, distance + 0.27],
                },
              },
              ...(slab
                ? [
                    {
                      name: "Slab Label",
                      _tags: { Company: [{ name: slab.company }] },
                      _identification: {
                        best_match: {
                          name: name.toUpperCase(),
                          grade: slab.grade,
                          certificate_number: slab.certificate,
                        },
                      },
                    },
                  ]
                : []),
            ],
          },
        ],
      },
    },
    grading:
      grade == null
        ? null
        : {
            endpoint: "grade",
            response: {
              records: [
                {
                  grades: {
                    corners: grade + 0.5,
                    edges: grade,
                    surface: grade - 1,
                    centering: Math.min(10, grade + 1),
                    final: grade,
                    condition,
                  },
                  card: [{ centering: { "left/right": "48/52", "top/bottom": "53/47" } }],
                },
                { grades: { final: grade - 0.5 } },
              ],
            },
          },
    gradingImages: [],
    calls: grade == null ? 2 : 3,
    analyzedAt: now(),
  };
}

/** What `select_match` does in Rust: the candidate swaps places with the best match. */
function selectMatch(id: number, index: number): ItemDetail {
  const item = [...items.values()].flat().find((i) => i.id === id)!;
  const ident: Json = matchOf(item.recognition?.response).ident;
  const [picked] = ident.alternatives.splice(index, 1);
  ident.alternatives.unshift(ident.best_match);
  ident.best_match = picked;
  const d: number[] = ident.distances;
  const [dist] = d.splice(index + 1, 1);
  d.splice(1, 0, d[0]);
  d[0] = dist;
  Object.assign(item, {
    name: picked.name,
    fullName: picked.full_name,
    setName: picked.set,
    setCode: picked.set_code,
    cardNumber: picked.card_number,
    year: String(picked.year ?? ""),
    distance: dist ?? null,
    confidence: tier(dist),
    price: representativePrice(picked.price_stats),
    picked: true,
    edited: false,
    reviewed: true,
    updatedAt: now(),
  });
  void emit("item-updated", summary(item));
  void emit("session-updated", session(item.sessionId));
  return item;
}

/** What `search_cards` does in Rust. */
function searchCards(query: string, filter: string, limit: number) {
  const q = query.trim().toLowerCase();
  const all = [...items.values()].flat().filter((i) => (i.status === "done" || i.status === "grading") && i.name);
  const key = (i: ItemDetail) => `${i.name}|${i.setName}|${i.cardNumber}`.toLowerCase();
  const copies = new Map<string, number>();
  for (const i of all) copies.set(key(i), (copies.get(key(i)) ?? 0) + 1);
  const found = all
    .map((i) => ({
      ...summary(i),
      sessionName: sessions.find((s) => s.id === i.sessionId)?.name ?? "",
      copies: copies.get(key(i)) ?? 1,
    }))
    .filter((c) =>
      filter === "review"
        ? needsReview(c)
        : filter === "graded"
          ? c.grade != null || c.slab != null
          : filter === "duplicates"
            ? c.copies > 1
            : true,
    )
    .filter(
      (c) =>
        !q ||
        [c.name, c.fullName, c.setName, c.cardNumber, c.location, c.notes].some((v) => v.toLowerCase().includes(q)),
    );
  return {
    cards: found.slice(0, limit),
    total: found.length,
    value: found.reduce((n, c) => n + (c.price ?? 0), 0),
  };
}

/** What `update_item` does in Rust (without the full name rebuilt). */
function updateItem(id: number, edit: ItemEdit): ItemDetail {
  const item = [...items.values()].flat().find((i) => i.id === id)!;
  const { notes, location, ...card } = edit;
  const changed = (Object.keys(card) as (keyof typeof card)[]).some((k) => (item[k] ?? "") !== (card[k] ?? ""));
  Object.assign(item, changed ? { ...card, edited: true } : {}, {
    notes: notes.trim(),
    location: location.trim(),
    reviewed: true,
    updatedAt: now(),
  });
  void emit("item-updated", summary(item));
  void emit("session-updated", session(item.sessionId));
  return item;
}

function changeItems(sessionId: string, ids: number[], change: (i: ItemDetail) => void): number {
  const list = (items.get(sessionId) ?? []).filter((i) => ids.includes(i.id));
  for (const i of list) {
    change(i);
    void emit("item-updated", summary(i));
  }
  void emit("session-updated", session(sessionId));
  return list.length;
}

const DETAIL_ONLY = [
  "eventId",
  "previewPath",
  "backPreviewPath",
  "recognition",
  "grading",
  "gradingImages",
  "calls",
  "analyzedAt",
  "remoteId",
  "ebaySku",
  "ebayOfferId",
  "ebayImages",
];

function summary(i: ItemDetail): ItemSummary {
  return Object.fromEntries(Object.entries(i).filter(([k]) => !DETAIL_ONLY.includes(k))) as unknown as ItemSummary;
}

function seed() {
  const base = {
    error: null,
    finishedAt: null,
    remoteSessionId: null,
    syncError: null,
    ebayError: null,
    ebayCheckedAt: null,
    counts: counts(""),
  };
  const options = {
    priceStats: true,
    slab: false,
    grading: "grade" as const,
    conditionMode: "ebay",
    pairing: "front_back" as const,
    bothSides: false,
    magicAi: false,
    customEndpoint: "",
    skipDetection: false,
    knownCategory: false,
    knownSide: false,
    knownRotation: false,
    alphabet: "",
    subcategory: "",
    scan: settings.scan,
    syncRemote: true,
    engine: "ximilar" as const,
    localModel: "lfm2.5-vl-3b",
  };
  sessions.push(
    {
      ...base,
      id: "demo-1",
      name: "Binder #3 — vintage Pokémon",
      sourceKind: "folder",
      sourceRef: "/Users/demo/Cards/binder3",
      idMode: "auto",
      options: { ...options, slab: true },
      status: "completed",
      createdAt: "2026-10-14T16:20:00Z",
      startedAt: "2026-10-14T16:20:00Z",
      remoteSessionId: "7c0e",
    },
    {
      ...base,
      id: "demo-3",
      name: "Shoebox — modern Magic",
      sourceKind: "folder",
      sourceRef: "/Users/demo/Cards/shoebox",
      idMode: "tcg",
      options: { ...options, syncRemote: false },
      status: "stopped",
      createdAt: "2026-10-12T09:05:00Z",
      startedAt: "2026-10-12T09:05:00Z",
      remoteSessionId: null,
    },
    {
      ...base,
      id: "demo-2",
      name: "fi-8170 batch — sports",
      sourceKind: "scanner",
      sourceRef: "fujitsu:fi-8170:1234",
      idMode: "sport",
      options: { ...options, bothSides: true, magicAi: true },
      status: "draft",
      createdAt: now(),
      startedAt: null,
    },
  );
  items.set(
    "demo-1",
    Array.from({ length: 18 }, (_, i) => makeItem("demo-1", i + 1)),
  );
  // A few of the binder's cards are on eBay already.
  const binder = items.get("demo-1")!;
  const listing = (status: EbayListing["status"], price: number | null, error: string | null = null) => ({
    status,
    price,
    listingId: status === "listed" || status === "sold" ? `1105530${binder.length}${Math.round(price ?? 0)}` : null,
    error,
  });
  binder[0].ebay = listing("listed", 27.5);
  binder[1].ebay = listing("draft", 18500);
  binder[3].ebay = listing("sold", 12);
  binder[4].ebay = listing(null, null, "Invalid category. (25005)");
  items.set("demo-2", []);
  items.set(
    "demo-3",
    Array.from({ length: 7 }, (_, i) => makeItem("demo-3", i + 1)),
  );
}

/** Simulates a run: cards arrive, get recognised, then graded. */
function simulate(id: string) {
  const s = sessions.find((x) => x.id === id)!;
  s.status = "running";
  void emit("session-updated", session(id));
  const list = items.get(id)!;
  let seq = list.length;
  const timer = setInterval(() => {
    if (s.status !== "running") return clearInterval(timer);
    seq += 1;
    const item = {
      ...makeItem(id, seq),
      status: "recognizing" as const,
      name: "",
      fullName: "",
      setName: "",
      cardNumber: "",
      confidence: null,
      price: null,
      grade: null,
      condition: null,
      synced: false,
    };
    list.push(item);
    void emit("item-updated", summary(item));
    setTimeout(() => {
      const done = { ...makeItem(id, seq), status: "grading" as const, grade: null, condition: null, synced: false };
      list[list.indexOf(item)] = done;
      void emit("item-updated", summary(done));
      void emit("session-updated", session(id));
      setTimeout(() => {
        const graded = makeItem(id, done.seq);
        list[list.findIndex((x) => x.id === done.id)] = graded;
        void emit("item-updated", summary(graded));
        void emit("session-updated", session(id));
      }, 1800);
    }, 900);
    void emit("session-updated", session(id));
    if (seq >= 24) {
      clearInterval(timer);
      setTimeout(() => {
        s.status = "completed";
        void emit("session-updated", session(id));
      }, 3000);
    }
  }, 700);
}

const remote = {
  id: "7c0e",
  name: "Binder #3 — vintage Pokémon",
  status: "closed",
  platform: "other",
  started: "2026-10-14T16:20:00Z",
  identification_count: 18,
  total_price: 19100,
  client: { name: "cardloader" },
};

const lfm = {
  licenseName: "LFM Open License v1.0",
  licenseUrl: "https://www.liquid.ai/lfm-license",
  downloaded: 0,
  state: "missing" as const,
  error: null,
  supported: true,
};
// The default first, as `local::catalog::MODELS`.
const models: ModelStatus[] = [
  {
    ...lfm,
    id: "lfm2.5-vl-3b",
    name: "LFM2.5-VL 3B",
    tier: "Accurate",
    params: "3B",
    memoryMb: 2500,
    size: 2_257_565_056,
    description:
      "Liquid AI's largest LFM2.5 vision model (Q4_K_M): the most reliable names and numbers, and the slowest.",
    homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-3B-GGUF",
  },
  {
    ...lfm,
    id: "lfm2.5-vl-450m",
    name: "LFM2.5-VL 450M",
    tier: "Fast",
    params: "450M",
    memoryMb: 600,
    size: 332_128_736,
    description:
      "Liquid AI's smallest vision language model (Q4_K_M). Quick and light; reads names well, collector numbers less reliably.",
    homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-450M-GGUF",
  },
  {
    ...lfm,
    id: "lfm2.5-vl-1.6b",
    name: "LFM2.5-VL 1.6B",
    tier: "Balanced",
    params: "1.6B",
    memoryMb: 1500,
    size: 1_314_006_144,
    description: "The 1.6B model (Q4_K_M): reads names and numbers clearly better than 450M, about 2–3 times slower.",
    homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF",
  },
];
models[0] = { ...models[0], state: "ready", downloaded: models[0].size };
const modelById = (id: string) => models.find((m) => m.id === id)!;

/** A download at ~40 MB/s. */
function downloadDemo(model: ModelStatus) {
  model.state = "downloading";
  const timer = setInterval(() => {
    if (model.state !== "downloading") return clearInterval(timer);
    model.downloaded = Math.min(model.size, model.downloaded + 10_000_000);
    if (model.downloaded >= model.size) {
      model.state = "ready";
      clearInterval(timer);
    }
    void emit("model-updated", { ...model });
  }, 250);
}

/** The local model reads each identified card; a few disagree. */
async function compareDemo(sessionId: string, runId: string, modelId: string) {
  const big = modelId !== "lfm2.5-vl-450m";
  const list = (items.get(sessionId) ?? []).filter((i) => i.status === "done");
  const rows = [];
  for (const [n, i] of list.entries()) {
    await new Promise((r) => setTimeout(r, 150));
    const nameMatches = big || n % 5 !== 3;
    const numberMatches = big ? n % 7 !== 2 : n % 3 !== 1;
    const row = {
      itemId: i.id,
      seq: i.seq,
      ximilar: i.fullName,
      local: `${nameMatches ? i.name : i.name.split(" ")[0]} #${numberMatches ? i.cardNumber : "330"}`,
      nameMatches,
      numberMatches,
      seconds: (big ? 2.6 : 0.8) + (n % 4) * 0.1,
      error: null,
    };
    rows.push(row);
    void emit("compare-progress", { runId, done: rows.length, total: list.length, row });
  }
  return {
    model: modelId,
    cards: rows.length,
    names: rows.filter((r) => r.nameMatches).length,
    numbers: rows.filter((r) => r.numberMatches).length,
    withNumber: rows.length,
    secondsPerCard: big ? 2.75 : 0.95,
    cancelled: false,
    rows,
  };
}

function ebayTitle(i: ItemDetail): string {
  const grade = i.slab ? ` ${i.slab.company} ${i.slab.grade}` : "";
  return `${i.year} ${i.setName} ${i.name} #${i.cardNumber}${grade}`.slice(0, 80);
}

function ebayPreview(i: ItemDetail): EbayPreview {
  // Pikachu has no Ximilar price here, so the dialog asks for one.
  const price = i.ebay?.price ?? (i.name === "Pikachu" ? null : i.price);
  return {
    itemId: i.id,
    seq: i.seq,
    thumbPath: i.thumbPath,
    title: ebayTitle(i),
    category: i.idType === "sport" ? "Sports Trading Card Singles" : "CCG Individual Cards",
    condition: i.slab ? `Graded · ${i.slab.company} ${i.slab.grade}` : "Ungraded",
    price,
    ebay: i.ebay,
    problem: null,
  };
}

/** An eBay job: one card every 300 ms, announced as the real one is. */
function ebayDemo(
  sessionId: string,
  action: EbayProgress["action"],
  cards: ItemDetail[],
  apply: (i: ItemDetail) => void,
) {
  const progress: EbayProgress = { sessionId, action, done: 0, total: cards.length, running: true };
  ebayJobs.set(sessionId, progress);
  void emit("ebay-progress", { ...progress });
  const timer = setInterval(() => {
    const card = cards[progress.done];
    if (card) {
      apply(card);
      card.updatedAt = now();
      void emit("item-updated", summary(card));
      progress.done += 1;
    }
    const finished = progress.done >= cards.length;
    if (finished) {
      clearInterval(timer);
      progress.running = false;
      ebayJobs.delete(sessionId);
    }
    void emit("ebay-progress", { ...progress });
    void emit("session-updated", session(sessionId));
  }, 300);
}

const ebayJobs = new Map<string, EbayProgress>();

/** What the Rust previews say for each format, roughly. */
function exportPreview(i: ItemDetail, format: string): EbayPreview {
  const p = ebayPreview(i);
  const sport = i.idType === "sport";
  if (format === "tcgplayer") {
    return {
      ...p,
      ebay: null,
      title: `${i.name} #${i.cardNumber}`,
      category: `${i.subcategory} · ${i.setName}`,
      condition: i.condition ? `${i.condition} (from grading)` : "Ungraded",
      problem: sport
        ? "TCGplayer doesn't sell sports cards"
        : i.slab
          ? "graded cards are sold as Listings with Photos on TCGplayer, one by one"
          : null,
    };
  }
  if (format === "shopify") {
    return { ...p, ebay: null, category: sport ? "Sports Trading Cards" : "Gaming Cards" };
  }
  const sub = sport ? `Sports Cards › ${i.subcategory} Singles` : `Trading Card Games › ${i.subcategory} Cards`;
  return { ...p, ebay: null, category: sub, price: p.price == null ? null : Math.ceil(p.price) };
}

/** Cards analysed per day over the last `days` days: scanning on most weekdays, rarely at weekends. */
function activity(days: number) {
  const out = [];
  const today = new Date();
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - i);
    const roll = (Math.sin(i * 12.9898) * 43758.5453) % 1;
    const weekend = d.getDay() === 0 || d.getDay() === 6;
    if (Math.abs(roll) < (weekend ? 0.8 : 0.35)) continue;
    const pad = (n: number) => String(n).padStart(2, "0");
    const cards = Math.round(Math.abs(roll) * 180) + 5;
    out.push({
      date: `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`,
      cards,
      calls: Math.round(cards * 2.3),
    });
  }
  return out;
}

export function installDemo() {
  seed();
  mockConvertFileSrc("macos");
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, never>;
      switch (cmd) {
        case "get_settings":
        case "set_token":
          return structuredClone(settings);
        case "save_settings": {
          const username = settings.ebay.username;
          Object.assign(settings, a.settings, { ebay: { ...(a.settings as unknown as SettingsView).ebay, username } });
          return structuredClone(settings);
        }
        case "set_ebay_cert":
          settings.ebayCertSet = !!String(a.certId ?? "").trim();
          return structuredClone(settings);
        case "ebay_connect":
        case "ebay_connect_with":
          return new Promise((resolve) =>
            setTimeout(() => {
              Object.assign(settings, { ebayConnected: true, ebay: { ...settings.ebay, username: "demo-seller" } });
              resolve(structuredClone(settings));
            }, 1500),
          );
        case "ebay_disconnect":
          Object.assign(settings, { ebayConnected: false, ebay: { ...settings.ebay, username: null } });
          return structuredClone(settings);
        case "ebay_setup":
        case "ebay_opt_in":
          return structuredClone(ebaySetup);
        case "ebay_create_location": {
          const key = `cardloader-${ebaySetup.locations.length + 1}`;
          ebaySetup.locations.push({ id: key, name: `Cardloader (${a.postalCode} ${a.country})` });
          settings.ebay.locationKey = key;
          return structuredClone(ebaySetup);
        }
        case "ebay_preview":
          return (items.get(a.sessionId) ?? []).filter((i) => i.status === "done").map(ebayPreview);
        case "ebay_create_drafts": {
          const prices = new Map(
            (a.items as unknown as { itemId: number; price: number }[]).map((d) => [d.itemId, d.price]),
          );
          const cards = (items.get(a.sessionId) ?? []).filter((i) => prices.has(i.id));
          ebayDemo(a.sessionId, "drafts", cards, (i) => {
            const live = i.ebay?.status === "listed";
            i.ebay = {
              status: live ? "listed" : "draft",
              price: prices.get(i.id)!,
              listingId: i.ebay?.listingId ?? null,
              error: null,
            };
            i.ebayOfferId = `offer-${i.id}`;
          });
          return null;
        }
        case "ebay_publish": {
          const ids = a.itemIds as unknown as number[] | null;
          const cards = (items.get(a.sessionId) ?? []).filter(
            (i) => i.ebay?.status === "draft" && (!ids || ids.includes(i.id)),
          );
          ebayDemo(a.sessionId, "publish", cards, (i) => {
            i.ebay = { ...i.ebay!, status: "listed", listingId: `1105${i.id}`, error: null };
          });
          return null;
        }
        case "ebay_refresh": {
          const s = sessions.find((x) => x.id === a.sessionId);
          if (s) s.ebayCheckedAt = now();
          const listed = (items.get(a.sessionId) ?? []).filter((i) => i.ebay?.status === "listed");
          // A forced check finds the first listing sold.
          ebayDemo(a.sessionId, "refresh", listed, (i) => {
            if (a.force && i === listed[0]) i.ebay = { ...i.ebay!, status: "sold" };
          });
          return null;
        }
        case "ebay_remove": {
          const ids = a.itemIds as unknown as number[];
          const cards = (items.get(a.sessionId) ?? []).filter((i) => ids.includes(i.id));
          ebayDemo(a.sessionId, "remove", cards, (i) => {
            if (i.ebay?.status === "sold") i.ebay = { ...i.ebay, error: "a sold card stays on eBay" };
            else Object.assign(i, { ebay: null, ebayOfferId: null, ebayImages: [] });
          });
          return null;
        }
        case "ebay_cancel":
        case "cancel_export":
          return null;
        case "plugin:dialog|save":
          return "/Users/demo/Cards/export.csv";
        case "set_imgbb_key":
          settings.imgbbKeySet = !!String(a.key ?? "").trim();
          return structuredClone(settings);
        case "export_preview":
          return (items.get(a.sessionId) ?? [])
            .filter((i) => i.status === "done")
            .map((i) => exportPreview(i, String(a.format)));
        case "export_tcgplayer": {
          const chosen = a.items as unknown as { itemId: number }[];
          const list = (items.get(a.sessionId) ?? []).filter((i) => chosen.some((c) => c.itemId === i.id));
          const out = list.filter((i) => i.idType !== "sport" && !i.slab);
          return {
            rows: out.length,
            skipped: list.filter((i) => !out.includes(i)).map((i) => ({ seq: i.seq, reason: "graded or sports" })),
          };
        }
        case "export_shopify": {
          const chosen = a.items as unknown as { itemId: number }[];
          const o = a.options as unknown as { uploadImages: boolean; file: string; grouping: string };
          return new Promise((resolve) => {
            let done = 0;
            const timer = setInterval(
              () => {
                done += 1;
                void emit("export-progress", { sessionId: a.sessionId, done, total: chosen.length });
                if (done >= chosen.length) {
                  clearInterval(timer);
                  const products = o.grouping === "by_card" ? Math.ceil(chosen.length / 2) : chosen.length;
                  resolve({
                    products,
                    variants: chosen.length - (o.grouping === "by_card" ? 2 : 0),
                    skipped: [],
                    withoutImages: o.uploadImages || o.file === "inventory" ? 0 : products,
                    imageError: null,
                  });
                }
              },
              o.uploadImages && o.file === "products" ? 150 : 10,
            );
          });
        }
        case "export_whatnot": {
          const chosen = a.items as unknown as { itemId: number }[];
          const upload = (a.options as unknown as { uploadImages: boolean }).uploadImages;
          return new Promise((resolve) => {
            let done = 0;
            const timer = setInterval(
              () => {
                done += 1;
                void emit("export-progress", { sessionId: a.sessionId, done, total: chosen.length });
                if (done >= chosen.length) {
                  clearInterval(timer);
                  resolve({
                    rows: chosen.length,
                    skipped: [],
                    withoutImages: upload ? 0 : chosen.length,
                    imageError: null,
                  });
                }
              },
              upload ? 150 : 10,
            );
          });
        }
        case "ebay_job":
          return ebayJobs.get(a.sessionId) ?? null;
        case "ebay_listing_url":
          return `https://sandbox.ebay.com/itm/${a.listingId}`;
        case "dashboard_activity":
          return activity(a.days);
        case "test_connection":
          return { ok: true, account: "demo@example.com", error: null, cardsCloud: true, cardsCloudError: null };
        case "list_sessions":
          return sessions.map((s) => session(s.id));
        case "get_session":
          return session(a.id);
        case "create_session": {
          const n = a.session as unknown as Session;
          const s: Session = {
            ...n,
            id: `demo-${sessions.length + 1}`,
            status: "draft",
            error: null,
            createdAt: now(),
            startedAt: null,
            finishedAt: null,
            remoteSessionId: a.remoteSession ? String(a.remoteSession) : null,
            syncError: null,
            ebayError: null,
            ebayCheckedAt: null,
            counts: counts(""),
          };
          sessions.unshift(s);
          items.set(s.id, []);
          return s;
        }
        case "rename_session":
        case "update_session": {
          const s = sessions.find((x) => x.id === a.id)!;
          Object.assign(
            s,
            { name: String(a.name).trim() },
            cmd === "update_session" ? { idMode: a.idMode, options: a.options } : {},
          );
          void emit("session-updated", session(s.id));
          return session(s.id);
        }
        case "delete_session": {
          const i = sessions.findIndex((x) => x.id === a.id);
          if (i >= 0) sessions.splice(i, 1);
          items.delete(a.id);
          return null;
        }
        case "preview_csv":
          return {
            rows: 26,
            cards: 20,
            cardsWithImages: 12,
            images: 5,
            skipped: 1,
            missingImages: 0,
            columns: ["Image → front image", "Card Name → name", "Set → set", "No. → number", "Price → price"],
          };
        case "import_csv": {
          const n = a.session as unknown as Session;
          const s: Session = {
            ...n,
            id: `demo-${sessions.length + 1}`,
            status: "completed",
            error: null,
            createdAt: now(),
            startedAt: null,
            finishedAt: now(),
            remoteSessionId: null,
            syncError: null,
            ebayError: null,
            ebayCheckedAt: null,
            counts: counts(""),
          };
          sessions.unshift(s);
          items.set(
            s.id,
            Array.from({ length: 20 }, (_, i) => ({
              ...makeItem(s.id, i + 1),
              grading: null,
              recognition: null,
              calls: 0,
              synced: false,
            })),
          );
          return session(s.id);
        }
        case "list_models":
          return models.map((m) => ({ ...m }));
        case "download_model":
          downloadDemo(modelById(a.id));
          return { ...modelById(a.id) };
        case "cancel_model_download": {
          const m = modelById(a.id);
          m.state = "missing";
          void emit("model-updated", { ...m });
          return null;
        }
        case "delete_model": {
          const m = modelById(a.id);
          Object.assign(m, { state: "missing", downloaded: 0 });
          void emit("model-updated", { ...m });
          return { ...m };
        }
        case "compare_local":
          return compareDemo(a.sessionId, a.runId, a.model ?? settings.localModel);
        case "cancel_compare":
          return null;
        case "session_folder":
          return "/Users/demo/Cards";
        case "export_csv":
          return (items.get(a.id) ?? []).length;
        case "rename_remote_session":
          return { ...remote, name: a.name };
        case "delete_remote_session":
          return null;
        case "get_remote_session":
          return { session: { ...remote, id: a.id }, summary: null };
        case "run_session":
          return simulate(a.id);
        case "pause_session":
        case "stop_session": {
          const s = sessions.find((x) => x.id === a.id)!;
          s.status = cmd === "pause_session" ? "paused" : "stopped";
          void emit("session-updated", session(a.id));
          return null;
        }
        case "list_items":
          return (items.get(a.sessionId) ?? []).map(summary);
        case "select_match":
          return selectMatch(a.id, a.index);
        case "get_item":
          return [...items.values()].flat().find((i) => i.id === a.id);
        case "search_cards":
          return searchCards(String(a.query ?? ""), a.filter, a.limit ?? 1000);
        case "update_item":
          return updateItem(a.id, a.edit);
        case "review_items":
          return changeItems(a.sessionId, a.itemIds, (i) => (i.reviewed = a.reviewed));
        case "locate_items":
          return changeItems(a.sessionId, a.itemIds, (i) => (i.location = String(a.location).trim()));
        case "delete_items": {
          const list = items.get(a.sessionId) ?? [];
          const ids = new Set<number>(a.itemIds);
          items.set(
            a.sessionId,
            list.filter((i) => !ids.has(i.id)),
          );
          void emit("session-updated", session(a.sessionId));
          return list.length - (items.get(a.sessionId) ?? []).length;
        }
        case "count_images":
          return 42;
        case "app_info":
          return {
            version: "0.3.0",
            dataDir: "/Users/demo/Library/Application Support/com.ximilar.cardloader",
            dataBytes: 734_003_200,
          };
        case "scanner_support":
          return { backend: "sane", available: true, hint: null };
        case "scanner_driver_settings":
          return true;
        case "list_scanners":
          return [{ id: "fujitsu:fi-8170:1234", name: "FUJITSU fi-8170", backend: "sane" }];
        case "list_remote_sessions": {
          const q = String(a.search ?? "").toLowerCase();
          const results = [remote].filter((r) => r.name.toLowerCase().includes(q));
          return { count: results.length, next: null, previous: null, results };
        }
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
