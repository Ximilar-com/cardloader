import { open } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import { Cloud, Cpu, Eye, FileSpreadsheet, FolderOpen, RefreshCw, ScanLine, Settings, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  api,
  errorText,
  type DeviceInfo,
  type IdMode,
  type NewSession,
  type Pairing,
  type CsvPreview,
  type Engine,
  type ScannerSupport,
  type Session,
  type SessionOptions,
  type SourceKind,
} from "../lib/api";
import { basename, defaultSessionName, GRADING_LABELS, ID_MODE_LABELS } from "../lib/format";
import { modelOption, useModels } from "./LocalModels";
import Modal, { ErrorNote } from "./Modal";
import Select, { optionsOf, type SelectOption } from "./Select";

const SOURCES: {
  kind: SourceKind;
  label: string;
  icon: typeof FolderOpen;
  text: string;
}[] = [
  {
    kind: "folder",
    label: "Folder",
    icon: FolderOpen,
    text: "Analyse the images in a folder",
  },
  {
    kind: "watch_folder",
    label: "Watch folder",
    icon: Eye,
    text: "Analyse images as another app saves them",
  },
  {
    kind: "scanner",
    label: "Scanner",
    icon: ScanLine,
    text: "Scan a stack of cards (e.g. fi-8170)",
  },
  {
    kind: "csv",
    label: "CSV file",
    icon: FileSpreadsheet,
    text: "Import cards or image paths from a spreadsheet",
  },
];

/**
 * Creates a session; with `from`, either edits that session's settings (`edit`, the source stays) or starts
 * a new session with the same source and settings (`duplicate`).
 */
export default function NewSessionDialog({
  onClose,
  from,
  mode = "new",
  onSaved,
  initialKind = "folder",
}: {
  onClose: () => void;
  from?: Session;
  mode?: "new" | "edit" | "duplicate";
  onSaved?: (session: Session) => void;
  initialKind?: SourceKind;
}) {
  const navigate = useNavigate();
  const editing = mode === "edit" && !!from;
  const [name, setName] = useState(() => (!from ? defaultSessionName() : editing ? from.name : `${from.name} (copy)`));
  const [kind, setKind] = useState<SourceKind>(from?.sourceKind ?? initialKind);
  const [folder, setFolder] = useState(
    from && ["folder", "watch_folder"].includes(from.sourceKind) ? from.sourceRef : "",
  );
  const [csvPath, setCsvPath] = useState(from?.sourceKind === "csv" ? from.sourceRef : "");
  const [csvInfo, setCsvInfo] = useState<{
    path: string;
    preview?: CsvPreview;
    error?: string;
  } | null>(null);
  const [reanalyze, setReanalyze] = useState(false);
  const [counted, setCounted] = useState<{ folder: string; n: number } | null>(null);
  const [device, setDevice] = useState(from?.sourceKind === "scanner" ? from.sourceRef : "");
  const [devices, setDevices] = useState<DeviceInfo[] | null>(null);
  const [support, setSupport] = useState<ScannerSupport | null>(null);
  const [scanning, setScanning] = useState(false);
  const [idMode, setIdMode] = useState<IdMode>(from?.idMode ?? "auto");
  const [options, setOptions] = useState<SessionOptions | null>(from?.options ?? null);
  const [tokenSet, setTokenSet] = useState(true);
  const [remoteSession, setRemoteSession] = useState("");
  const models = useModels();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.getSettings().then((s) => {
      setTokenSet(s.tokenSet);
      if (from) return;
      setIdMode(s.defaultIdMode);
      setFolder(s.defaultFolder ?? "");
      setDevice(s.defaultScanner ?? "");
      setOptions({
        priceStats: s.priceStats,
        slab: false,
        grading: s.defaultGrading,
        conditionMode: "ebay",
        pairing: s.defaultPairing,
        bothSides: false,
        magicAi: false,
        customEndpoint: "",
        skipDetection: false,
        knownCategory: false,
        knownSide: false,
        knownRotation: false,
        alphabet: "",
        subcategory: "",
        scan: s.scan,
        syncRemote: s.syncByDefault,
        engine: s.engine,
        localModel: s.localModel,
      });
    });
  }, [from]);

  useEffect(() => {
    if (!folder || kind === "scanner" || kind === "csv") return;
    let alive = true;
    api
      .countImages(folder)
      .then((n) => alive && setCounted({ folder, n }))
      .catch(() => alive && setCounted(null));
    return () => {
      alive = false;
    };
  }, [folder, kind]);
  const imageCount = counted?.folder === folder ? counted.n : null;

  useEffect(() => {
    if (!csvPath || kind !== "csv") return;
    let alive = true;
    api
      .previewCsv(csvPath)
      .then((preview) => alive && setCsvInfo({ path: csvPath, preview }))
      .catch((e) => alive && setCsvInfo({ path: csvPath, error: errorText(e) }));
    return () => {
      alive = false;
    };
  }, [csvPath, kind]);
  const csv = csvInfo?.path === csvPath ? csvInfo : null;

  const pickCsv = async () => {
    const picked = await open({
      filters: [{ name: "CSV", extensions: ["csv", "tsv", "txt"] }],
      defaultPath: csvPath || undefined,
    });
    if (typeof picked === "string") {
      setCsvPath(picked);
      if (!from) setName(basename(picked).replace(/\.[^.]+$/, ""));
    }
  };

  const findScanners = () => {
    setScanning(true);
    setError(null);
    api
      .listScanners()
      .then((list) => {
        setDevices(list);
        if (list.length && !list.some((d) => d.id === device)) setDevice(list[0].id);
      })
      .catch((e) => setError(errorText(e)))
      .finally(() => setScanning(false));
  };

  useEffect(() => {
    if (kind !== "scanner" || support) return;
    api.scannerSupport().then((s) => {
      setSupport(s);
      if (s.available) findScanners();
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind]);

  const chooseSource = (k: SourceKind) => {
    setKind(k);
    // A duplex scanner gives front + back of every card.
    if (k === "scanner" && options?.scan.duplex) setOptions((o) => o && { ...o, pairing: "front_back" });
  };

  const pickFolder = async () => {
    const picked = await open({
      directory: true,
      defaultPath: folder || undefined,
    });
    if (typeof picked === "string") setFolder(picked);
  };

  const set = <K extends keyof SessionOptions>(key: K, value: SessionOptions[K]) =>
    setOptions((o) => o && { ...o, [key]: value });

  const csvBlocked = kind === "csv" && !csv?.preview;
  // What the source still needs, highlighted until it is filled in.
  const needsFolder = !editing && (kind === "folder" || kind === "watch_folder") && !folder.trim();
  const needsCsv = !editing && kind === "csv" && !csvPath.trim();
  const sourceMissing = needsFolder || needsCsv;
  const local = options?.engine === "local";
  // Only a downloaded model can be picked: the chosen one, else the first downloaded.
  const ready = models?.filter((m) => m.state === "ready") ?? [];
  const model = ready.find((m) => m.id === options?.localModel) ?? ready[0];
  const chosen = options && local && model ? { ...options, localModel: model.id } : options;
  // A local session needs Ximilar only to sync.
  const needsToken = !local || !!options?.syncRemote;
  const canRun = (!needsToken || tokenSet) && (!local || !!model);

  const save = async () => {
    if (!chosen || !from) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await api.updateSession(from.id, name, idMode, chosen);
      onSaved?.(saved);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  const create = async (run: boolean) => {
    if (!chosen) return;
    setBusy(true);
    setError(null);
    const session: NewSession = {
      name,
      sourceKind: kind,
      sourceRef: kind === "scanner" ? device : kind === "csv" ? csvPath : folder,
      idMode,
      options: chosen,
    };
    try {
      const created =
        kind === "csv"
          ? await api.importCsv(session, reanalyze)
          : await api.createSession(session, remoteSession.trim());
      if (run) await api.runSession(created.id);
      onClose();
      navigate(`/sessions/${created.id}`);
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      title={editing ? "Session settings" : mode === "duplicate" ? "Duplicate session" : "New session"}
      onClose={onClose}
      footer={
        editing ? (
          <>
            <button className="btn" onClick={onClose}>
              Cancel
            </button>
            <button className="btn btn-primary" onClick={save} disabled={busy || !options}>
              Save
            </button>
          </>
        ) : (
          <>
            <button
              className="btn"
              onClick={() => create(false)}
              disabled={busy || !options || csvBlocked || sourceMissing}
            >
              {kind === "csv" ? (busy ? "Importing…" : "Import") : "Create"}
            </button>
            <button
              className="btn btn-primary"
              onClick={() => create(true)}
              disabled={busy || !options || !canRun || csvBlocked || sourceMissing}
              title={kind === "csv" ? "Import, then analyse the images and upload as set below" : undefined}
            >
              {kind === "csv" ? "Import & run" : "Create & run"}
            </button>
          </>
        )
      }
    >
      <div className="space-y-5">
        {needsToken && !tokenSet && (
          <ErrorNote>
            <div className="flex items-center gap-3">
              <span className="flex-1">Set your Ximilar API token in Settings before running a session.</span>
              <OpenSettings section="ximilar" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        {local && models && !model && (
          <ErrorNote>
            <div className="flex items-center gap-3">
              <span className="flex-1">Download a local model in Settings before running this session.</span>
              <OpenSettings section="models" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        {editing && (
          <p className="rounded-md bg-slate-50 px-3 py-2 text-sm text-slate-600 dark:bg-slate-900 dark:text-slate-300">
            Changes apply to the cards analysed from now on. Cards already analysed keep their results; use{" "}
            <b>Duplicate</b> to analyse the same source again with other settings.
          </p>
        )}
        <div>
          <label className="label">Name</label>
          <input className="input" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
        </div>

        <div>
          <label className="label">Source</label>
          <div className="grid grid-cols-4 gap-2">
            {SOURCES.map(({ kind: k, label, icon: Icon, text }) => (
              <button
                key={k}
                onClick={() => chooseSource(k)}
                disabled={editing && kind !== k}
                className={clsx(
                  "flex flex-col items-start gap-1 rounded-lg border p-3 text-left transition-colors disabled:opacity-40",
                  kind === k
                    ? "border-accent bg-accent/5 ring-2 ring-accent/20"
                    : "border-slate-200 enabled:hover:border-slate-300 dark:border-slate-700",
                )}
              >
                <Icon className="h-5 w-5 text-accent" />
                <span className="text-sm font-medium">{label}</span>
                <span className="text-xs text-slate-500">{text}</span>
              </button>
            ))}
          </div>
        </div>

        {kind === "csv" ? (
          <div className="space-y-2">
            <label className="label">CSV file</label>
            <div className="flex gap-2">
              <input
                className={clsx("input", needsCsv && MISSING)}
                value={csvPath}
                onChange={(e) => setCsvPath(e.target.value)}
                placeholder="/path/to/cards.csv"
                disabled={editing}
              />
              {!editing && (
                <button className={clsx("btn", needsCsv && "btn-primary")} onClick={pickCsv}>
                  <FileSpreadsheet className="h-4 w-4" /> Browse
                </button>
              )}
            </div>
            {!csvPath && !editing && (
              <p className="text-xs text-slate-500">
                One card per row. Rows with a card name (and set, number, price, grade…) are imported as identified
                cards; rows with only an image path are analysed. A cardloader CSV export imports back as it was.
              </p>
            )}
            {csv?.error && <ErrorNote>{csv.error}</ErrorNote>}
            {csv?.preview && !editing && <CsvSummary preview={csv.preview} />}
            {csv?.preview && csv.preview.cardsWithImages > 0 && !editing && (
              <Check
                label={`Analyse the images of the ${csv.preview.cardsWithImages} identified rows again`}
                checked={reanalyze}
                onChange={setReanalyze}
              />
            )}
          </div>
        ) : kind !== "scanner" ? (
          <div>
            <label className="label">Folder</label>
            <div className="flex gap-2">
              <input
                className={clsx("input", needsFolder && MISSING)}
                value={folder}
                onChange={(e) => setFolder(e.target.value)}
                placeholder="/path/to/cards"
                disabled={editing}
              />
              {!editing && (
                <button className={clsx("btn", needsFolder && "btn-primary")} onClick={pickFolder}>
                  <FolderOpen className="h-4 w-4" /> Browse
                </button>
              )}
            </div>
            {needsFolder ? (
              <p className="mt-1 text-xs text-amber-700 dark:text-amber-400">
                {kind === "watch_folder"
                  ? "Choose the folder your scanning app saves images into."
                  : "Choose the folder with your card images."}
              </p>
            ) : (
              imageCount != null && (
                <p
                  className={clsx(
                    "mt-1 text-xs",
                    imageCount === 0 && kind === "folder" ? "text-amber-700 dark:text-amber-400" : "text-slate-500",
                  )}
                >
                  {imageCount === 0 && kind === "folder"
                    ? "No images in this folder (JPEG, PNG, WebP, TIFF or BMP)."
                    : `${imageCount} image${imageCount === 1 ? "" : "s"} found`}
                  {kind === "watch_folder" && " — new images will be picked up while the session runs"}
                </p>
              )
            )}
          </div>
        ) : (
          <div className="space-y-3">
            {support && !support.available && <ErrorNote>{support.hint}</ErrorNote>}
            <div>
              <label className="label">Scanner</label>
              <div className="flex gap-2">
                <Select
                  value={device}
                  onChange={setDevice}
                  disabled={editing}
                  aria-label="Scanner"
                  options={
                    devices?.length
                      ? devices.map((d) => ({ value: d.id, label: d.name || d.id, icon: ScanLine }))
                      : [
                          {
                            value: device,
                            label: device || (scanning ? "Searching…" : "No scanner found"),
                            icon: ScanLine,
                          },
                        ]
                  }
                />
                <button className="btn" onClick={findScanners} disabled={editing || scanning || !support?.available}>
                  <RefreshCw className={clsx("h-4 w-4", scanning && "animate-spin")} />
                </button>
              </div>
            </div>
            {options && (
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className="label">Resolution</label>
                  <Select
                    value={String(options.scan.dpi)}
                    onChange={(v) => set("scan", { ...options.scan, dpi: Number(v) })}
                    options={DPI_OPTIONS}
                  />
                </div>
                <div>
                  <label className="label">Sides</label>
                  <Select
                    value={options.scan.duplex ? "duplex" : "front"}
                    onChange={(v) => {
                      const duplex = v === "duplex";
                      setOptions({
                        ...options,
                        scan: { ...options.scan, duplex },
                        pairing: duplex ? "front_back" : "single",
                      });
                    }}
                    options={[
                      { value: "duplex", label: "Front + back" },
                      { value: "front", label: "Front only" },
                    ]}
                  />
                </div>
                <div>
                  <label className="label">Multifeed detection</label>
                  <Select
                    value={options.scan.multifeed}
                    onChange={(v) => set("scan", { ...options.scan, multifeed: v })}
                    options={MULTIFEED_OPTIONS}
                  />
                </div>
              </div>
            )}
          </div>
        )}

        {options && (
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="label">Identify with</label>
              <Select value={options.engine} onChange={(v) => set("engine", v)} options={ENGINE_OPTIONS} />
            </div>
            {local ? (
              <div>
                <label className="label">Model</label>
                <Select
                  value={model?.id ?? ""}
                  onChange={(v) => set("localModel", v)}
                  placeholder="No model downloaded"
                  options={(models ?? []).map((m) =>
                    m.state === "ready"
                      ? modelOption(m)
                      : { ...modelOption(m), disabled: true, hint: "not downloaded" },
                  )}
                />
              </div>
            ) : (
              <div />
            )}
            {local && (
              <p className="col-span-2 -mt-2 text-xs text-slate-500">
                Runs on this computer: reads the name, collector number and year printed on each card. No prices, other
                candidates or grading.
              </p>
            )}
            <div className={local ? "hidden" : undefined}>
              <label className="label">Identify as</label>
              <Select value={idMode} onChange={setIdMode} options={optionsOf(ID_MODE_LABELS)} />
            </div>
            <div className={local ? "hidden" : undefined}>
              <label className="label">Grading</label>
              <Select value={options.grading} onChange={(v) => set("grading", v)} options={optionsOf(GRADING_LABELS)} />
            </div>
            {kind !== "csv" && (
              <div>
                <label className="label">Images per card</label>
                <Select
                  value={options.pairing}
                  onChange={(pairing) =>
                    setOptions({ ...options, pairing, bothSides: pairing === "front_back" && options.bothSides })
                  }
                  options={PAIRING_OPTIONS}
                />
              </div>
            )}
            {!local && options.grading === "condition" ? (
              <div>
                <label className="label">Condition scale</label>
                <Select
                  value={options.conditionMode}
                  onChange={(v) => set("conditionMode", v)}
                  options={[
                    { value: "ebay", label: "eBay" },
                    { value: "tcgplayer", label: "TCGplayer" },
                    { value: "cardmarket", label: "Cardmarket" },
                    { value: "ximilar", label: "Ximilar" },
                  ]}
                />
              </div>
            ) : (
              kind !== "csv" && <div />
            )}
            {!local && (
              <>
                <Check label="Price statistics" checked={options.priceStats} onChange={(v) => set("priceStats", v)} />
                <Check label="Read graded slab labels" checked={options.slab} onChange={(v) => set("slab", v)} />
                <KnownAttributes
                  options={options}
                  idMode={idMode}
                  onChange={(patch) => setOptions({ ...options, ...patch })}
                />
                <BothSides
                  options={options}
                  idMode={idMode}
                  csv={kind === "csv"}
                  onChange={(patch) => setOptions({ ...options, ...patch })}
                />
              </>
            )}
            <Check
              label="Sync to Ximilar Cards Cloud"
              checked={options.syncRemote || !!remoteSession.trim()}
              disabled={!!remoteSession.trim()}
              onChange={(v) => set("syncRemote", v)}
            />
            {!editing && kind !== "csv" && (
              <ContinueRemote
                value={remoteSession}
                onChange={setRemoteSession}
                onFound={(remoteName) => setName(remoteName)}
                folder={kind !== "scanner"}
              />
            )}
          </div>
        )}
        <ErrorNote>{error}</ErrorNote>
      </div>
    </Modal>
  );
}

const CATEGORY_OF: Partial<Record<IdMode, string>> = {
  tcg: "Card/Trading Card Game",
  sport: "Card/Sport Card",
  comics: "Comics",
  slab: "Slab Label",
};

const TYPE_NOUN: Partial<Record<IdMode, string>> = {
  tcg: "a trading card",
  sport: "a sports card",
  comics: "a comic",
  slab: "a graded slab",
};

const ALPHABET_OPTIONS: SelectOption<string>[] = [
  { value: "", label: "Detect", hint: "Ximilar reads it from the card" },
  { value: "latin", label: "Latin", hint: "English, European languages" },
  { value: "japanese", label: "Japanese" },
  { value: "chinese", label: "Chinese" },
  { value: "korean", label: "Korean" },
  { value: "russian", label: "Russian" },
  { value: "thai", label: "Thai" },
];

// Subcategory values as Ximilar's collectibles taxonomy spells them (mirrors recognition::TCG_GAMES / SPORTS).
const GAME_LABELS: Record<string, string> = { Pokemon: "Pokémon", "YuGiOh!": "Yu-Gi-Oh!", Lotr: "Lord of the Rings" };
const TCG_GAMES = [
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
const SPORTS = ["Baseball", "Basketball", "Football", "Hockey", "MMA", "Soccer"];

const subcategoryOptions = (values: string[], any: string): SelectOption<string>[] => [
  { value: "", label: any, hint: "Ximilar reads it from the card" },
  ...values.map((v) => ({ value: v, label: GAME_LABELS[v] ?? v })).sort((a, b) => a.label.localeCompare(b.label)),
];
const GAME_OPTIONS = subcategoryOptions(TCG_GAMES, "Any game");
const SPORT_OPTIONS = subcategoryOptions(SPORTS, "Any sport");

/** What is already known about every card, so Ximilar skips working it out: faster and more accurate. */
function KnownAttributes({
  options,
  idMode,
  onChange,
}: {
  options: SessionOptions;
  idMode: IdMode;
  onChange: (patch: Partial<SessionOptions>) => void;
}) {
  const category = CATEGORY_OF[idMode];
  return (
    <div className="col-span-2 space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
      <div>
        <div className="flex items-center gap-1.5 text-sm font-medium">
          <Zap className="h-4 w-4 text-accent" /> Faster and more accurate
        </div>
        <p className="mt-0.5 text-xs text-slate-500">
          Tell Ximilar what you already know about the cards, so it skips working it out. Check only what is true of
          every image: a wrong value makes matches worse.
        </p>
      </div>
      <Hinted
        label="Skip detection"
        checked={options.skipDetection}
        onChange={(v) => onChange({ skipDetection: v })}
        hint="For images already cropped to the card: each image is sent as the card itself, so Ximilar doesn’t look for it in the picture."
        warning={
          options.skipDetection && options.slab
            ? "Slab labels are found by detection, so they may not be read."
            : undefined
        }
      />
      <Hinted
        label={category ? `Every card is ${TYPE_NOUN[idMode]}` : "Card type"}
        checked={!!category && options.knownCategory}
        disabled={!category}
        onChange={(v) => onChange({ knownCategory: v })}
        hint={
          category
            ? `Sends Category “${category}”.`
            : "Choose a type in “Identify as” first; Auto-detect leaves it to Ximilar."
        }
      />
      <Hinted
        label="Front side"
        checked={options.knownSide}
        onChange={(v) => onChange({ knownSide: v })}
        hint="The images show the front (backs sent with “front and the back” are marked as the back)."
      />
      <Hinted
        label="Upright"
        checked={options.knownRotation}
        onChange={(v) => onChange({ knownRotation: v })}
        hint="No card is rotated or upside down."
      />
      {idMode === "tcg" && (
        <div className="grid grid-cols-2 gap-3">
          <div>
            <div className="label">Game</div>
            <Select
              value={TCG_GAMES.includes(options.subcategory) ? options.subcategory : ""}
              onChange={(subcategory) => onChange({ subcategory })}
              options={GAME_OPTIONS}
              aria-label="Game"
            />
          </div>
          <div>
            <div className="label">Alphabet</div>
            <Select
              value={options.alphabet}
              onChange={(alphabet) => onChange({ alphabet })}
              options={ALPHABET_OPTIONS}
              aria-label="Alphabet"
            />
          </div>
          <p className="col-span-2 -mt-1 text-xs text-slate-500">
            The game narrows the search to its cards; the alphabet (e.g. Japanese for Japanese prints) keeps cards from
            matching another language’s version.
            {TCG_GAMES.includes(options.subcategory) && !options.alphabet && (
              <span className="text-amber-700 dark:text-amber-400">
                {" "}
                With a game set, Ximilar no longer reads the alphabet and assumes Latin: choose it for prints in other
                scripts.
              </span>
            )}
          </p>
        </div>
      )}
      {idMode === "sport" && (
        <div>
          <div className="label">Sport</div>
          <Select
            value={SPORTS.includes(options.subcategory) ? options.subcategory : ""}
            onChange={(subcategory) => onChange({ subcategory })}
            options={SPORT_OPTIONS}
            aria-label="Sport"
          />
          <p className="mt-1 text-xs text-slate-500">Narrows the search to that sport’s cards.</p>
        </div>
      )}
    </div>
  );
}

/** A checkbox with an explanation below it. */
function Hinted({
  label,
  checked,
  onChange,
  hint,
  warning,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  hint: string;
  warning?: string;
  disabled?: boolean;
}) {
  return (
    <div className="space-y-0.5">
      <Check label={label} checked={checked} onChange={onChange} disabled={disabled} />
      <p className="pl-6 text-xs text-slate-500">
        {hint}
        {warning && <span className="text-amber-700 dark:text-amber-400"> {warning}</span>}
      </p>
    </div>
  );
}

/** Front + back identification, magic AI and a custom endpoint: mostly for sports cards. */
function BothSides({
  options,
  idMode,
  csv,
  onChange,
}: {
  options: SessionOptions;
  idMode: IdMode;
  csv: boolean;
  onChange: (patch: Partial<SessionOptions>) => void;
}) {
  const custom = options.customEndpoint.trim() !== "";
  const readsBacks = custom || idMode === "sport" || idMode === "auto";
  const sport = idMode === "sport" || idMode === "auto";
  return (
    <div className="col-span-2 space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
      <div className="space-y-1">
        <Check
          label="Identify from the front and the back"
          checked={options.bothSides}
          // The back has to be paired with its front first.
          onChange={(v) => onChange({ bothSides: v, ...(v && !csv ? { pairing: "front_back" } : {}) })}
        />
        <p className="pl-6 text-xs text-slate-500">
          {csv ? (
            "Uses the back image column of the CSV."
          ) : (
            <>
              Images named <code>card_F.jpg</code> and <code>card_B.jpg</code> (or front/back) are paired by name,
              otherwise consecutive images.
            </>
          )}{" "}
          The back goes to recognition with its front — this helps sports cards, whose backs carry the player, year and
          number.
          {options.bothSides && !readsBacks && (
            <span className="text-amber-700 dark:text-amber-400">
              {" "}
              {ID_MODE_LABELS[idMode]} identification reads the front only: their backs all look alike.
            </span>
          )}
        </p>
      </div>
      <div className="space-y-1">
        <Check label="Magic AI for sports cards" checked={options.magicAi} onChange={(v) => onChange({ magicAi: v })} />
        <p className="pl-6 text-xs text-slate-500">
          When a sports card isn’t in Ximilar’s database, an AI model identifies it from its images instead. Extra API
          credits are used only when it runs.
          {options.magicAi && !sport && (
            <span className="text-amber-700 dark:text-amber-400"> Applies to sports cards only.</span>
          )}
        </p>
      </div>
      {idMode === "auto" && !custom && (options.bothSides || options.magicAi) && (
        <p className="text-xs text-slate-500">
          Auto-detect identifies a card in one call; a sports card is then sent to Sports ID as well for these options
          (one more call).
        </p>
      )}
      <div>
        <label className="label">Custom endpoint (optional)</label>
        <input
          className="input font-mono text-xs"
          value={options.customEndpoint}
          onChange={(e) => onChange({ customEndpoint: e.target.value })}
          placeholder="collectibles/v2/…"
          spellCheck={false}
        />
        <p className="mt-1 text-xs text-slate-500">
          A recognition endpoint Ximilar set up for you. It gets the same request as the standard one (front and back
          when chosen above), and its answers are read as{" "}
          {idMode === "auto" ? "the detected card type" : `“${ID_MODE_LABELS[idMode]}”`}.
        </p>
      </div>
    </div>
  );
}

const MISSING = "border-amber-400 ring-2 ring-amber-400/25 dark:border-amber-500";

export const ENGINE_OPTIONS: SelectOption<Engine>[] = [
  { value: "ximilar", label: "Ximilar API", icon: Cloud },
  { value: "local", label: "Local model", icon: Cpu },
];

export const DPI_OPTIONS: SelectOption<string>[] = [
  { value: "300", label: "300 dpi", hint: "identification" },
  { value: "600", label: "600 dpi", hint: "grading" },
];

export const MULTIFEED_OPTIONS: SelectOption<"off" | "default">[] = [
  { value: "off", label: "Off", hint: "recommended for cards" },
  { value: "default", label: "Driver default" },
];

export const PAIRING_OPTIONS: SelectOption<Pairing>[] = [
  { value: "single", label: "One image per card" },
  { value: "front_back", label: "Front + back pairs" },
];

/** Opens Settings at one of its tabs. */
function OpenSettings({ section, onOpen }: { section: string; onOpen: () => void }) {
  const navigate = useNavigate();
  return (
    <button
      className="btn shrink-0 py-1 text-xs"
      onClick={() => {
        onOpen();
        navigate("/settings", { state: { section } });
      }}
    >
      <Settings className="h-3.5 w-3.5" /> Open Settings
    </button>
  );
}

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi;

/** The ID of a Cards Cloud session to add to, from the ID or a link to it; looked up as it is typed. */
function ContinueRemote({
  value,
  onChange,
  onFound,
  folder,
}: {
  value: string;
  onChange: (v: string) => void;
  onFound: (name: string) => void;
  folder: boolean;
}) {
  const id = value.match(UUID)?.pop()?.toLowerCase() ?? null;
  const [found, setFound] = useState<{ id: string; text: string; ok: boolean } | null>(null);

  useEffect(() => {
    if (!id) return;
    let alive = true;
    const t = setTimeout(() => {
      api
        .getRemoteSession(id)
        .then(({ session: r }) => {
          if (!alive) return;
          const n = r.identification_count;
          setFound({ id, ok: true, text: `“${r.name || "Untitled"}” · ${n} card${n === 1 ? "" : "s"}` });
          if (r.name) onFound(r.name);
        })
        .catch((e) => alive && setFound({ id, ok: false, text: errorText(e) }));
    }, 300);
    return () => {
      alive = false;
      clearTimeout(t);
    };
    // onFound is a new function each render; the lookup depends on the ID only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const shown = found?.id === id ? found : null;
  const skips = folder ? "; images it has already (matched by file name) are skipped" : "";
  return (
    <div className="col-span-2">
      <label className="label">Continue a Cards Cloud session (optional)</label>
      <input
        className="input font-mono text-xs"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Session ID or link"
        spellCheck={false}
      />
      <p className={clsx("mt-1 text-xs", shown && !shown.ok ? "text-red-600" : "text-slate-500")}>
        {value.trim() && !id
          ? "Paste the session’s ID or its link from the Ximilar App."
          : shown
            ? shown.ok
              ? `${shown.text}. New cards are added to it${skips}.`
              : shown.text
            : id
              ? "Looking it up…"
              : `Adds the cards to a session started earlier, e.g. on another computer${skips}.`}
      </p>
    </div>
  );
}

function CsvSummary({ preview: p }: { preview: CsvPreview }) {
  const n = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;
  return (
    <div className="rounded-md bg-slate-50 px-3 py-2 text-xs text-slate-600 dark:bg-slate-900 dark:text-slate-300">
      <p>
        <b>{n(p.rows, "row")}</b>: {n(p.cards, "identified card")} to import, {n(p.images, "image")} to analyse
        {p.skipped > 0 && `, ${p.skipped} empty skipped`}.
        {p.missingImages > 0 && (
          <span className="text-amber-700 dark:text-amber-400">
            {" "}
            {n(p.missingImages, "row")} point to missing images.
          </span>
        )}
      </p>
      <p className="mt-1 text-slate-500">Columns: {p.columns.join(", ")}</p>
    </div>
  );
}

export function Check({
  label,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className={clsx("flex items-center gap-2 text-sm", disabled ? "opacity-70" : "cursor-pointer")}>
      <input
        type="checkbox"
        className="h-4 w-4 accent-[var(--color-accent)]"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      {label}
    </label>
  );
}
