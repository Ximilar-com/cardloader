// Browser preview: `npm run dev` outside Tauri answers the Rust commands with demo data, so the UI can be
// developed (and reviewed) in a plain browser. Never part of a production build.

import { emit } from "@tauri-apps/api/event";
import { mockConvertFileSrc, mockIPC } from "@tauri-apps/api/mocks";
import type { ItemDetail, ItemSummary, Json, ModelStatus, Session, SettingsView } from "./api";
import { matchOf, representativePrice, tier } from "./cardview";

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
  defaultSources: ["scanner", "folder"],
  // The demo starts with the setup, as a first launch does.
  setupDone: false,
  priceStats: true,
  concurrency: 4,
  syncByDefault: true,
  engine: "ximilar",
  localModel: "lfm2.5-vl-450m",
  tokenSet: true,
  tokenStorage: "keychain",
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
    updatedAt: now(),
    eventId: `demo${seq}`,
    remoteId: null,
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
    updatedAt: now(),
  });
  void emit("item-updated", summary(item));
  void emit("session-updated", session(item.sessionId));
  return item;
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
];

function summary(i: ItemDetail): ItemSummary {
  return Object.fromEntries(Object.entries(i).filter(([k]) => !DETAIL_ONLY.includes(k))) as unknown as ItemSummary;
}

function seed() {
  const base = { error: null, finishedAt: null, remoteSessionId: null, syncError: null, counts: counts("") };
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
    localModel: "lfm2.5-vl-450m",
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
const models: ModelStatus[] = [
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
    description:
      "The newer 1.6B model (Q4_K_M): reads names and numbers clearly better than 450M, about 2–3 times slower.",
    homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF",
  },
  {
    ...lfm,
    id: "lfm2-vl-1.6b",
    name: "LFM2-VL 1.6B",
    tier: "Balanced",
    params: "1.6B",
    memoryMb: 2000,
    size: 1_810_368_096,
    description:
      "The first-generation 1.6B model (Q8_0). Kept for comparison: LFM2.5-VL 1.6B is more accurate and faster.",
    homepage: "https://huggingface.co/LiquidAI/LFM2-VL-1.6B-GGUF",
  },
  {
    ...lfm,
    id: "lfm2-vl-3b",
    name: "LFM2-VL 3B",
    tier: "Accurate",
    params: "3B",
    memoryMb: 2400,
    size: 2_150_437_632,
    description: "The largest model (Q4_K_M): the most reliable names and numbers, and the slowest.",
    homepage: "https://huggingface.co/LiquidAI/LFM2-VL-3B-GGUF",
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
    out.push({
      date: `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`,
      cards: Math.round(Math.abs(roll) * 180) + 5,
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
        case "save_settings":
          Object.assign(settings, a.settings);
          return settings;
        case "get_settings":
        case "set_token":
          return settings;
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
        case "count_images":
          return 42;
        case "scanner_support":
          return { backend: "sane", available: true, hint: null };
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
