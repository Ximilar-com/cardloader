import { open } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import {
  BadgeCheck,
  Bot,
  Check as CheckMark,
  ChevronRight,
  Cloud,
  Cpu,
  Crop,
  DollarSign,
  Eye,
  FileSpreadsheet,
  FolderOpen,
  Gauge,
  RefreshCw,
  ScanLine,
  Search,
  Settings,
  ShoppingBag,
  Zap,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
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
  type GradingMode,
  type Prices,
  type ScannerSupport,
  type Session,
  type SessionOptions,
  type SourceKind,
  type Task,
  type CropFormat,
} from "../lib/api";
import { locale, rich, t, useT } from "../i18n";
import {
  basename,
  chosenPrices,
  defaultSessionName,
  engineLabels,
  engineOf,
  gradeEngineOf,
  gradingLabels,
  agentOf,
  cropsOf,
  gradingOf,
  idModeLabels,
  pricesOf,
  sourceLabels,
  TASK_CHOICES,
  taskLabels,
  tasksOf,
  usesApi,
  withGradeMode,
  withTasks,
  type TaskChoice,
} from "../lib/format";
import { modelOption, useModels } from "./LocalModels";
import Modal, { ErrorNote } from "./Modal";
import Select, { optionsOf, type SelectOption } from "./Select";

const SOURCES: { kind: SourceKind; icon: typeof FolderOpen }[] = [
  { kind: "folder", icon: FolderOpen },
  { kind: "watch_folder", icon: Eye },
  { kind: "scanner", icon: ScanLine },
  { kind: "csv", icon: FileSpreadsheet },
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
  const t = useT();
  const navigate = useNavigate();
  const editing = mode === "edit" && !!from;
  const [name, setName] = useState(() =>
    !from ? defaultSessionName() : editing ? from.name : t("newSession.copyName", { name: from.name }),
  );
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
  const [defaultPairing, setDefaultPairing] = useState<Pairing>("single");
  /** "Images per card" as the user chose it: in this dialog, or for the session it copies (unless a scanner or
   * reading backs set it there). */
  const [chosenPairing, setChosenPairing] = useState<Pairing | null>(
    from && from.sourceKind !== "scanner" && !from.options.bothSides ? from.options.pairing : null,
  );
  const [options, setOptions] = useState<SessionOptions | null>(from?.options ?? null);
  const [tokenSet, setTokenSet] = useState(true);
  /** Whether agents can connect and run sessions (Settings → AI agents): an agent session needs both. */
  const [agentsReady, setAgentsReady] = useState(true);
  /** Whether eBay's listings can be searched (Settings → Selling): prices from eBay need it. */
  const [ebaySearch, setEbaySearch] = useState(true);
  /** The task tab shown below the source; falls back to the first chosen task. */
  const [tab, setTab] = useState<TaskChoice>("identify");
  const [remoteSession, setRemoteSession] = useState("");
  const models = useModels();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.getSettings().then((s) => {
      setTokenSet(s.tokenSet);
      setAgentsReady(s.mcp.enabled && s.mcp.allowRun);
      setEbaySearch(s.ebaySearch);
      // Scanner sessions always scan with the settings in Settings → Scanner (see `saveScan`).
      if (from?.sourceKind === "scanner") {
        setOptions((o) => o && { ...o, scan: s.scan, pairing: s.scan.duplex ? "front_back" : "single" });
      }
      setDefaultPairing(s.defaultPairing);
      // A scanner session keeps its scanner.
      if (from?.sourceKind !== "scanner") setDevice(s.defaultScanner ?? "");
      if (from) return;
      setIdMode(s.defaultIdMode);
      setFolder(s.defaultFolder ?? "");
      const options: SessionOptions = {
        task: s.defaultTask,
        saveCrops: s.defaultSaveCrops,
        cropFolder: s.defaultCropFolder ?? "",
        cropLevel: true,
        cropFormat: s.defaultCropFormat,
        priceStats: s.priceStats,
        slab: false,
        grading: s.defaultGrading,
        conditionMode: s.defaultConditionMode,
        // A duplex scanner gives front + back of every card (as `chooseSource` does).
        pairing: initialKind === "scanner" && s.scan.duplex ? "front_back" : s.defaultPairing,
        bothSides: false,
        magicAi: false,
        customEndpoint: "",
        skipDetection: false,
        knownCategory: false,
        knownSide: false,
        knownRotation: false,
        alphabet: "",
        subcategory: s.defaultSubcategory,
        scan: s.scan,
        syncRemote: s.syncByDefault,
        engine: s.engine,
        // Each task starts with the default engine: an agent identifies but never grades, and Ximilar's prices
        // come only with its identification.
        gradeEngine: s.engine === "agent" ? "ximilar" : s.engine,
        prices: s.priceStats && s.engine === "ximilar" ? "ximilar" : "off",
        localModel: s.localModel,
      };
      setOptions(withKindDefaults(options, s.defaultIdMode, options.pairing));
    });
  }, [from, initialKind]);

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
      .then(setDevices)
      .catch((e) => setError(errorText(e)))
      .finally(() => setScanning(false));
  };
  // Whichever of the settings and the scanner list comes first.
  const scanner = scannerOf(devices, device);

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
    setOptions((o) => o && { ...o, pairing: pairingAfterSource(o, kind, k, chosenPairing ?? defaultPairing) });
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
  const task: Task = options?.task ?? "identify";
  const identifies = task === "identify";
  /** Whether the session edited identifies its cards, once it has some. */
  const lockedKind = editing && from && from.counts.total > 0 ? from.options.task === "identify" : null;
  const chosenTasks = options ? tasksOf(options) : [];
  const shownTab = chosenTasks.includes(tab) ? tab : chosenTasks[0];
  const savesCrops = !!options && cropsOf(options);
  // Crops go to a folder of the user's: the session's own folder would be deleted with the session.
  const needsCropFolder = savesCrops && !options?.cropFolder.trim();
  const sourceMissing = needsFolder || needsCsv || needsCropFolder;
  const grading = options ? gradingOf(options) : "off";
  // Each task by its own engine; cropping always runs on the Ximilar API.
  const identifiesLocally = identifies && options?.engine === "local";
  const gradeEngine = options ? gradeEngineOf(options) : "ximilar";
  const gradesLocally = grading !== "off" && gradeEngine === "local";
  const usesLocal = identifiesLocally || gradesLocally;
  const prices: Prices = options ? pricesOf(options) : "off";
  // An AI agent connected over MCP identifies the cards, or prices them.
  const agent = !!options && agentOf(options);
  const agentPrices = prices === "agent";
  const ebayPrices = prices === "ebay";
  // A local session holds one kind of cards (Auto and slabs are the API's).
  const sessionIdMode = identifiesLocally ? localKind(idMode) : idMode;
  // What "Images per card" goes back to when a local session stops reading backs.
  const basePairing: Pairing =
    kind === "scanner" && options?.scan.duplex ? "front_back" : (chosenPairing ?? defaultPairing);
  const chooseTasks = (next: TaskChoice[]) => {
    if (!next.length) return;
    setOptions((o) => o && withTasks(o, next));
  };
  const toggleTask = (k: TaskChoice) =>
    chooseTasks(
      chosenTasks.includes(k)
        ? chosenTasks.filter((c) => c !== k)
        : TASK_CHOICES.filter((c) => c === k || chosenTasks.includes(c)),
    );
  // Ximilar's prices come only with its identification: identifying another way prices from eBay instead.
  const chooseEngine = (engine: Engine) =>
    setOptions((o) => {
      if (!o) return o;
      const priced: SessionOptions =
        engine !== "ximilar" && chosenPrices(o) === "ximilar" ? { ...o, prices: "ebay", priceStats: false } : o;
      return o.engine === "local" || engine === "local"
        ? withKindDefaults({ ...priced, engine }, idMode, basePairing)
        : { ...priced, engine };
    });
  const choosePrices = (p: Prices) => set("prices", p);
  const pickCropFolder = async () => {
    const picked = await open({ directory: true, defaultPath: options?.cropFolder || folder || undefined });
    if (typeof picked === "string") set("cropFolder", picked);
  };
  const chooseIdMode = (mode: IdMode) => {
    setIdMode(mode);
    if (identifiesLocally) setOptions((o) => o && withKindDefaults(o, mode, basePairing));
  };
  // Only grading and cropping use more than a 300 dpi scan (the runner's `scan_settings`).
  const grades = savesCrops || (grading !== "off" && !gradesLocally);
  // Only a downloaded model can be picked: the chosen one, else the first downloaded.
  const ready = models?.filter((m) => m.state === "ready") ?? [];
  const model = ready.find((m) => m.id === options?.localModel) ?? ready[0];
  const picked = options && usesLocal && model ? { ...options, localModel: model.id } : options;
  // What each task does is written out: prices and the grading engine as they are read, only identified cards go
  // to Cards Cloud, and only identifying can be an agent's.
  const chosen =
    picked &&
    ({
      ...picked,
      gradeEngine: gradeEngineOf(picked),
      prices: pricesOf(picked),
      priceStats: pricesOf(picked) === "ximilar",
      ...(identifies ? {} : { syncRemote: false, engine: engineOf(picked) }),
    } satisfies SessionOptions);
  // A session whose tasks all run elsewhere needs no Ximilar token.
  const needsToken = !options || usesApi(options);
  const canRun =
    (!needsToken || tokenSet) &&
    (!usesLocal || !!model) &&
    (!(agent || agentPrices) || agentsReady) &&
    (!ebayPrices || ebaySearch);

  // The scanner row edits Settings → Scanner, which every scanner run uses; the next session that crops starts in
  // this one's folder, with its file type.
  const saveScan = async () => {
    if (!chosen) return;
    const scan = kind === "scanner";
    const cropFolder = savesCrops && chosen.cropFolder.trim();
    if (!scan && !cropFolder) return;
    const settings = await api.getSettings();
    await api.saveSettings({
      ...settings,
      ...(scan ? { scan: chosen.scan } : {}),
      ...(cropFolder ? { defaultCropFolder: cropFolder, defaultCropFormat: chosen.cropFormat } : {}),
    });
  };

  const save = async () => {
    if (!chosen || !from) return;
    setBusy(true);
    setError(null);
    try {
      await saveScan();
      const saved = await api.updateSession(from.id, name, sessionIdMode, chosen);
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
      sourceRef: kind === "scanner" ? scanner : kind === "csv" ? csvPath : folder,
      idMode: sessionIdMode,
      options: chosen,
    };
    try {
      await saveScan();
      const created =
        kind === "csv"
          ? await api.importCsv(session, reanalyze)
          : await api.createSession(session, identifies ? remoteSession.trim() : "");
      if (run) await api.runSession(created.id);
      onClose();
      navigate(`/sessions/${created.id}`);
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  /** Where the crops go and how they look: for a crop-only session, or one that also saves crops. */
  const cropFields = (options: SessionOptions) => (
    <>
      <div className="col-span-2">
        <label className="label">{t("newSession.crop.folder")}</label>
        <div className="flex gap-2">
          <input
            className={clsx("input", needsCropFolder && MISSING)}
            value={options.cropFolder}
            onChange={(e) => set("cropFolder", e.target.value)}
            placeholder="/path/to/cropped"
          />
          <button className={clsx("btn", needsCropFolder && "btn-primary")} onClick={pickCropFolder}>
            <FolderOpen className="h-4 w-4" /> {t("newSession.browse")}
          </button>
        </div>
        <p className={clsx("mt-1 text-xs", needsCropFolder ? "text-amber-700 dark:text-amber-400" : "text-slate-500")}>
          {needsCropFolder ? t("newSession.crop.choose") : t("newSession.crop.hint")}
        </p>
      </div>
      <div>
        <label className="label">{t("newSession.crop.format")}</label>
        <Select value={options.cropFormat} onChange={(v) => set("cropFormat", v)} options={cropFormatOptions()} />
      </div>
      <div className="col-span-2">
        <Check label={t("newSession.crop.level")} checked={options.cropLevel} onChange={(v) => set("cropLevel", v)} />
        <p className="mt-1 pl-6 text-xs text-slate-500">{t("newSession.crop.levelHint")}</p>
      </div>
    </>
  );

  /** The local model, for whichever task runs on this computer (one model does both). */
  const modelField = (
    <div>
      <label className="label">{t("newSession.model")}</label>
      <Select
        value={model?.id ?? ""}
        onChange={(v) => set("localModel", v)}
        placeholder={t("newSession.noModel")}
        options={(models ?? []).map((m) =>
          m.state === "ready"
            ? modelOption(m)
            : { ...modelOption(m), disabled: true, hint: t("newSession.notDownloaded") },
        )}
      />
    </div>
  );

  return (
    <Modal
      title={t(
        editing
          ? "newSession.title.edit"
          : mode === "duplicate"
            ? "newSession.title.duplicate"
            : "newSession.title.new",
      )}
      onClose={onClose}
      footer={
        editing ? (
          <>
            <button className="btn" onClick={onClose}>
              {t("common.cancel")}
            </button>
            <button className="btn btn-primary" onClick={save} disabled={busy || !options || needsCropFolder}>
              {t("common.save")}
            </button>
          </>
        ) : (
          <>
            <button
              className="btn"
              onClick={() => create(false)}
              disabled={busy || !options || csvBlocked || sourceMissing}
            >
              {kind === "csv" ? (busy ? t("newSession.importing") : t("newSession.import")) : t("newSession.create")}
            </button>
            <button
              className="btn btn-primary"
              onClick={() => create(true)}
              disabled={busy || !options || !canRun || csvBlocked || sourceMissing}
              title={kind === "csv" ? t("newSession.importAndRunTitle") : undefined}
            >
              {kind === "csv" ? t("newSession.importAndRun") : t("newSession.createAndRun")}
            </button>
          </>
        )
      }
    >
      <div className="space-y-5">
        {needsToken && !tokenSet && (
          <ErrorNote>
            <div className="flex items-center gap-3">
              <span className="flex-1">{t("newSession.needsToken")}</span>
              <OpenSettings section="ximilar" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        {(agent || agentPrices) && !agentsReady && (
          <ErrorNote>
            <div className="flex items-center gap-3">
              <span className="flex-1">{t(agent ? "newSession.needsAgents" : "newSession.needsAgentsPrices")}</span>
              <OpenSettings section="agents" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        {ebayPrices && !ebaySearch && (
          <ErrorNote>
            <div className="flex items-center gap-3">
              <span className="flex-1">{t("newSession.needsEbay")}</span>
              <OpenSettings section="ebay" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        {usesLocal && models && !model && (
          <ErrorNote>
            <div className="flex items-center gap-3">
              <span className="flex-1">{t("newSession.needsModel")}</span>
              <OpenSettings section="models" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        {editing && (
          <p className="rounded-md bg-slate-50 px-3 py-2 text-sm text-slate-600 dark:bg-slate-900 dark:text-slate-300">
            {rich(t("newSession.editNote"), { b: (s) => <b>{s}</b> })}
          </p>
        )}
        <div>
          <label className="label">{t("newSession.name")}</label>
          <input className="input" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
        </div>

        {options && (
          <div>
            <label className="label">{t("newSession.whatToDo")}</label>
            <div className="grid grid-cols-4 gap-2">
              {TASK_CHOICES.map((k) => {
                const on = chosenTasks.includes(k);
                const Icon = TASK_ICONS[k];
                // A session with cards keeps identifying them, or keeps not: `update_session` refuses the change.
                const locked = k === "identify" && lockedKind != null;
                const needsIdentify = k === "prices" && !identifies;
                const last = on && chosenTasks.length === 1;
                return (
                  <button
                    key={k}
                    onClick={() => toggleTask(k)}
                    disabled={locked || needsIdentify}
                    aria-pressed={on}
                    title={locked ? t("newSession.identifyLocked") : last ? t("newSession.keepOne") : undefined}
                    className={clsx(
                      "relative flex flex-col items-start gap-1 rounded-lg border p-3 text-left transition-colors disabled:opacity-40",
                      on
                        ? "border-accent bg-accent/5 ring-2 ring-accent/20"
                        : "border-slate-200 enabled:hover:border-slate-300 dark:border-slate-700",
                    )}
                  >
                    <span
                      className={clsx(
                        "absolute top-2.5 right-2.5 flex h-4 w-4 items-center justify-center rounded border",
                        on ? "border-accent bg-accent text-white" : "border-slate-300 dark:border-slate-600",
                      )}
                    >
                      {on && <CheckMark className="h-3 w-3" strokeWidth={3} />}
                    </span>
                    <Icon className="h-5 w-5 text-accent" />
                    <span className="text-sm font-medium">{t(`newSession.tasks.${k}`)}</span>
                    <span className="text-xs text-slate-500">
                      {needsIdentify ? t("newSession.needsIdentify") : t(`newSession.taskHints.${k}`)}
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        <div>
          <label className="label">{t("newSession.source")}</label>
          <div className="grid grid-cols-4 gap-2">
            {SOURCES.map(({ kind: k, icon: Icon }) => (
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
                <span className="text-sm font-medium">{sourceLabels()[k]}</span>
                <span className="text-xs text-slate-500">{t(`newSession.sources.${k}`)}</span>
              </button>
            ))}
          </div>
        </div>

        {kind === "csv" ? (
          <div className="space-y-2">
            <label className="label">{t("newSession.csv.label")}</label>
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
                  <FileSpreadsheet className="h-4 w-4" /> {t("newSession.browse")}
                </button>
              )}
            </div>
            {!csvPath && !editing && <p className="text-xs text-slate-500">{t("newSession.csv.help")}</p>}
            {csv?.error && <ErrorNote>{csv.error}</ErrorNote>}
            {csv?.preview && !editing && <CsvSummary preview={csv.preview} />}
            {csv?.preview && csv.preview.cardsWithImages > 0 && !editing && (
              <Check
                label={t("newSession.csv.reanalyze", { count: csv.preview.cardsWithImages })}
                checked={reanalyze}
                onChange={setReanalyze}
              />
            )}
          </div>
        ) : kind !== "scanner" ? (
          <div>
            <label className="label">{t("newSession.folder.label")}</label>
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
                  <FolderOpen className="h-4 w-4" /> {t("newSession.browse")}
                </button>
              )}
            </div>
            {needsFolder ? (
              <p className="mt-1 text-xs text-amber-700 dark:text-amber-400">
                {kind === "watch_folder" ? t("newSession.folder.chooseWatch") : t("newSession.folder.choose")}
              </p>
            ) : (
              imageCount != null && (
                <p
                  className={clsx(
                    "mt-1 text-xs",
                    imageCount === 0 && kind === "folder" ? "text-amber-700 dark:text-amber-400" : "text-slate-500",
                  )}
                >
                  {kind === "watch_folder"
                    ? t("newSession.folder.foundWatching", { count: imageCount })
                    : imageCount === 0
                      ? t("newSession.folder.noImages")
                      : t("newSession.folder.found", { count: imageCount })}
                  {imageCount > 0 && options && kind === "folder" && ` · ${costOf(options, imageCount)}`}
                </p>
              )
            )}
          </div>
        ) : (
          <div className="space-y-3">
            {support && !support.available && <ErrorNote>{support.hint}</ErrorNote>}
            <div>
              <label className="label">{t("newSession.scanner.label")}</label>
              <div className="flex gap-2">
                <Select
                  value={scanner}
                  onChange={setDevice}
                  disabled={editing}
                  aria-label={t("newSession.scanner.label")}
                  options={
                    devices?.length
                      ? devices.map((d) => ({ value: d.id, label: d.name || d.id, icon: ScanLine }))
                      : [
                          {
                            value: scanner,
                            label:
                              scanner ||
                              (scanning ? t("newSession.scanner.searching") : t("newSession.scanner.noneFound")),
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
                  <label className="label">{t("newSession.scanner.resolution")}</label>
                  <Select
                    value={grades ? String(options.scan.dpi) : "300"}
                    onChange={(v) => set("scan", { ...options.scan, dpi: Number(v) })}
                    options={dpiOptions()}
                    disabled={!grades}
                  />
                  {!grades && (
                    <p className="mt-1 text-xs text-slate-500">{t("newSession.scanner.dpiWhenNotGrading")}</p>
                  )}
                </div>
                <div>
                  <label className="label">{t("newSession.scanner.sides")}</label>
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
                      { value: "duplex", label: t("newSession.scanner.duplex") },
                      { value: "front", label: t("newSession.scanner.frontOnly") },
                    ]}
                  />
                </div>
                <div>
                  <label className="label">{t("newSession.scanner.multifeed")}</label>
                  <Select
                    value={options.scan.multifeed}
                    onChange={(v) => set("scan", { ...options.scan, multifeed: v })}
                    options={multifeedOptions()}
                  />
                </div>
                <p className="col-span-3 -mt-1 text-xs text-slate-500">{t("newSession.scanner.shared")}</p>
              </div>
            )}
          </div>
        )}

        {options && kind !== "csv" && kind !== "scanner" && (
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="label">{t("newSession.imagesPerCard")}</label>
              <Select
                value={options.pairing}
                onChange={(pairing) => {
                  setChosenPairing(pairing);
                  setOptions({ ...options, pairing, bothSides: pairing === "front_back" && options.bothSides });
                }}
                options={pairingOptions()}
              />
            </div>
          </div>
        )}

        {options && shownTab && (
          <div>
            <div role="tablist" className="flex gap-1 border-b border-slate-200 dark:border-slate-700">
              {chosenTasks.map((k) => (
                <button
                  key={k}
                  role="tab"
                  aria-selected={shownTab === k}
                  onClick={() => setTab(k)}
                  className={clsx(
                    "-mb-px flex items-baseline gap-1.5 border-b-2 px-3 py-2 text-sm",
                    shownTab === k
                      ? "border-accent font-medium text-slate-900 dark:text-white"
                      : "border-transparent text-slate-500 hover:text-slate-800 dark:hover:text-slate-200",
                  )}
                >
                  {t(`newSession.tasks.${k}`)}
                  <span className="text-xs font-normal text-slate-400">
                    {k === "identify"
                      ? engineLabels()[options.engine]
                      : k === "prices"
                        ? t(`status.prices.${prices}`)
                        : k === "grade"
                          ? engineLabels()[gradeEngine]
                          : engineLabels().ximilar}
                  </span>
                </button>
              ))}
            </div>

            <div role="tabpanel" className="grid grid-cols-2 gap-4 pt-4">
              {shownTab === "identify" && (
                <>
                  <div>
                    <label className="label">{t("newSession.identifyWith")}</label>
                    <Select value={options.engine} onChange={chooseEngine} options={engineOptions(true)} />
                  </div>
                  {identifiesLocally ? modelField : <div />}
                  {identifiesLocally && (
                    <p className="col-span-2 -mt-2 text-xs text-slate-500">{t("newSession.localNote")}</p>
                  )}
                  {agent && <p className="col-span-2 -mt-2 text-xs text-slate-500">{t("newSession.agentNote")}</p>}
                  <div>
                    <label className="label">{t("newSession.identifyAs")}</label>
                    <Select
                      value={sessionIdMode}
                      onChange={chooseIdMode}
                      options={identifiesLocally ? localKindOptions() : optionsOf(idModeLabels())}
                    />
                  </div>
                  <div />
                  {/* A comic is read off its cover: its back says nothing about it. */}
                  {identifiesLocally && sessionIdMode !== "comics" && (
                    <div className="col-span-2">
                      <Check
                        label={t("newSession.localBack.label")}
                        checked={options.bothSides}
                        onChange={(v) =>
                          setOptions({
                            ...options,
                            bothSides: v,
                            ...(kind === "csv" ? {} : { pairing: v ? "front_back" : options.pairing }),
                          })
                        }
                      />
                      <p className="mt-1 pl-6 text-xs text-slate-500">{t("newSession.localBack.hint")}</p>
                    </div>
                  )}
                  {options.engine === "ximilar" && (
                    <>
                      <Check
                        label={t("newSession.readSlabs")}
                        checked={options.slab}
                        onChange={(v) => set("slab", v)}
                      />
                      <div />
                      <MoreOptions initiallyOpen={usesMoreOptions(options)}>
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
                      </MoreOptions>
                    </>
                  )}
                  <Check
                    label={t("newSession.syncRemote")}
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
                </>
              )}

              {shownTab === "prices" && (
                <>
                  <div>
                    <label className="label">{t("newSession.priceWith")}</label>
                    <Select
                      value={prices === "off" ? chosenPrices(options) : prices}
                      onChange={choosePrices}
                      options={priceOptions(options.engine === "ximilar")}
                    />
                  </div>
                  <div />
                  {prices !== "off" && (
                    <p className="col-span-2 -mt-2 text-xs text-slate-500">{t(`newSession.priceNotes.${prices}`)}</p>
                  )}
                </>
              )}

              {shownTab === "grade" && (
                <>
                  <div>
                    <label className="label">{t("newSession.gradeWith")}</label>
                    <Select
                      value={gradeEngine}
                      onChange={(v) => set("gradeEngine", v)}
                      options={engineOptions(false)}
                    />
                  </div>
                  {gradesLocally && !identifiesLocally ? modelField : <div />}
                  {gradesLocally && identifiesLocally && (
                    <p className="col-span-2 -mt-2 text-xs text-slate-500">{t("newSession.sameModel")}</p>
                  )}
                  <div>
                    <label className="label">{t("newSession.gradeWhat")}</label>
                    <Select
                      value={grading === "off" ? "grade" : grading}
                      onChange={(mode) => setOptions(withGradeMode(options, mode))}
                      options={gradeModeOptions()}
                    />
                  </div>
                  {grading === "condition" || (gradesLocally && grading === "grade") ? (
                    <div>
                      <label className="label">{t("newSession.conditionScale")}</label>
                      <Select
                        value={conditionScale(options.conditionMode, gradesLocally)}
                        onChange={(v) => set("conditionMode", v)}
                        options={gradesLocally ? localConditionOptions() : conditionOptions()}
                      />
                    </div>
                  ) : (
                    <div />
                  )}
                </>
              )}

              {shownTab === "crop" && (
                <>
                  {cropFields(options)}
                  <p className="col-span-2 -mt-2 text-xs text-slate-500">{t("newSession.cropNote")}</p>
                </>
              )}
            </div>
          </div>
        )}
        <ErrorNote>{error}</ErrorNote>
      </div>
    </Modal>
  );
}

/** What a local session can hold; any other "Identify as" reads as trading cards. */
const LOCAL_KINDS: IdMode[] = ["tcg", "sport", "comics"];

function localKind(mode: IdMode): IdMode {
  return LOCAL_KINDS.includes(mode) ? mode : "tcg";
}

function localKindOptions(): SelectOption<IdMode>[] {
  const labels = idModeLabels();
  return LOCAL_KINDS.map((value) => ({ value, label: labels[value] }));
}

/** A local session of sports cards reads both sides by default (the number, year and set are on the back); other
 * kinds, and the API, start with the front only. Turning the back off returns "Images per card" to `basePairing`. */
function withKindDefaults(o: SessionOptions, mode: IdMode, basePairing: Pairing): SessionOptions {
  const back = o.engine === "local" && o.task === "identify" && localKind(mode) === "sport";
  if (back === o.bothSides) return o;
  return { ...o, bothSides: back, pairing: back ? "front_back" : basePairing };
}

/** "Images per card" after the source changes: a duplex scanner gives front + back of every card, and leaving the
 * scanner goes back to `base` (the user's choice, else the default), or front + back while the session reads the
 * backs. Any other change keeps it. */
export function pairingAfterSource(o: SessionOptions, from: SourceKind, to: SourceKind, base: Pairing): Pairing {
  if (to === "scanner" && o.scan.duplex) return "front_back";
  if (from === "scanner" && to !== "scanner") return o.bothSides ? "front_back" : base;
  return o.pairing;
}

/** The scanner a session uses: the wanted one (the default, or the session's) when it was found, else the first
 * found. */
export function scannerOf(devices: DeviceInfo[] | null, wanted: string): string {
  return devices?.length && !devices.some((d) => d.id === wanted) ? devices[0].id : wanted;
}

const CATEGORY_OF: Partial<Record<IdMode, string>> = {
  tcg: "Card/Trading Card Game",
  sport: "Card/Sport Card",
  comics: "Comics",
  slab: "Slab Label",
};

function alphabetOptions(): SelectOption<string>[] {
  return [
    { value: "", label: t("newSession.alphabets.detect"), hint: t("newSession.subcategory.readFromCard") },
    { value: "latin", label: t("newSession.alphabets.latin"), hint: t("newSession.alphabets.latinHint") },
    { value: "japanese", label: t("newSession.alphabets.japanese") },
    { value: "chinese", label: t("newSession.alphabets.chinese") },
    { value: "korean", label: t("newSession.alphabets.korean") },
    { value: "russian", label: t("newSession.alphabets.russian") },
    { value: "thai", label: t("newSession.alphabets.thai") },
  ];
}

// Subcategory values as Ximilar's collectibles taxonomy spells them (mirrors recognition::TCG_GAMES / SPORTS).
export const GAME_LABELS: Record<string, string> = {
  Pokemon: "Pokémon",
  "YuGiOh!": "Yu-Gi-Oh!",
  Lotr: "Lord of the Rings",
};
export const TCG_GAMES = [
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
  { value: "", label: any, hint: t("newSession.subcategory.readFromCard") },
  ...values
    .map((v) => ({ value: v, label: GAME_LABELS[v] ?? v }))
    .sort((a, b) => a.label.localeCompare(b.label, locale())),
];
export const gameOptions = () => subcategoryOptions(TCG_GAMES, t("newSession.subcategory.anyGame"));
const sportOptions = () => subcategoryOptions(SPORTS, t("newSession.subcategory.anySport"));

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
  const t = useT();
  const category = CATEGORY_OF[idMode];
  return (
    <div className="col-span-2 space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
      <div>
        <div className="flex items-center gap-1.5 text-sm font-medium">
          <Zap className="h-4 w-4 text-accent" /> {t("newSession.known.title")}
        </div>
        <p className="mt-0.5 text-xs text-slate-500">{t("newSession.known.intro")}</p>
      </div>
      <Hinted
        label={t("newSession.known.skipDetection")}
        checked={options.skipDetection}
        onChange={(v) => onChange({ skipDetection: v })}
        hint={t("newSession.known.skipDetectionHint")}
        warning={options.skipDetection && options.slab ? t("newSession.known.skipDetectionSlab") : undefined}
      />
      <Hinted
        label={
          category && idMode !== "auto" ? t(`newSession.known.everyCard.${idMode}`) : t("newSession.known.cardType")
        }
        checked={!!category && options.knownCategory}
        disabled={!category}
        onChange={(v) => onChange({ knownCategory: v })}
        hint={category ? t("newSession.known.sendsCategory", { category }) : t("newSession.known.chooseTypeFirst")}
      />
      <Hinted
        label={t("newSession.known.frontSide")}
        checked={options.knownSide}
        onChange={(v) => onChange({ knownSide: v })}
        hint={t("newSession.known.frontSideHint")}
      />
      <Hinted
        label={t("newSession.known.upright")}
        checked={options.knownRotation}
        onChange={(v) => onChange({ knownRotation: v })}
        hint={t("newSession.known.uprightHint")}
      />
      {idMode === "tcg" && (
        <div className="grid grid-cols-2 gap-3">
          <div>
            <div className="label">{t("newSession.known.game")}</div>
            <Select
              value={TCG_GAMES.includes(options.subcategory) ? options.subcategory : ""}
              onChange={(subcategory) => onChange({ subcategory })}
              options={gameOptions()}
              aria-label={t("newSession.known.game")}
            />
          </div>
          <div>
            <div className="label">{t("newSession.known.alphabet")}</div>
            <Select
              value={options.alphabet}
              onChange={(alphabet) => onChange({ alphabet })}
              options={alphabetOptions()}
              aria-label={t("newSession.known.alphabet")}
            />
          </div>
          <p className="col-span-2 -mt-1 text-xs text-slate-500">
            {t("newSession.known.gameNote")}
            {TCG_GAMES.includes(options.subcategory) && !options.alphabet && (
              <span className="text-amber-700 dark:text-amber-400"> {t("newSession.known.gameNoAlphabet")}</span>
            )}
          </p>
        </div>
      )}
      {idMode === "sport" && (
        <div>
          <div className="label">{t("newSession.known.sport")}</div>
          <Select
            value={SPORTS.includes(options.subcategory) ? options.subcategory : ""}
            onChange={(subcategory) => onChange({ subcategory })}
            options={sportOptions()}
            aria-label={t("newSession.known.sport")}
          />
          <p className="mt-1 text-xs text-slate-500">{t("newSession.known.sportNote")}</p>
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

/** Known attributes, front and back, Magic AI and a custom endpoint: folded away unless one is in use. */
function MoreOptions({ initiallyOpen, children }: { initiallyOpen: boolean; children: ReactNode }) {
  const t = useT();
  const [open, setOpen] = useState(initiallyOpen);
  return (
    <div className="col-span-2 space-y-3">
      <button
        type="button"
        className="flex max-w-full items-center gap-1 text-left text-sm font-medium text-slate-600 hover:text-slate-900 dark:text-slate-300 dark:hover:text-white"
        onClick={() => setOpen(!open)}
        aria-expanded={open}
      >
        <ChevronRight className={clsx("h-4 w-4 shrink-0 transition-transform", open && "rotate-90")} />
        <span className="shrink-0">{t("newSession.more.label")}</span>
        {!open && <span className="min-w-0 truncate font-normal text-slate-400">{t("newSession.more.summary")}</span>}
      </button>
      {open && <div className="grid grid-cols-2 gap-4">{children}</div>}
    </div>
  );
}

function usesMoreOptions(o: SessionOptions): boolean {
  return (
    o.skipDetection ||
    o.knownCategory ||
    o.knownSide ||
    o.knownRotation ||
    !!o.alphabet ||
    !!o.subcategory ||
    o.bothSides ||
    o.magicAi ||
    !!o.customEndpoint.trim()
  );
}

/** Roughly what analysing the folder costs: a call per card to identify it with Ximilar, one more to grade it there
 * and one more to crop it; Cards Cloud bills one per stored image and one for the session. Local, agent and eBay
 * stages cost no API calls. */
function costOf(o: SessionOptions, images: number): string {
  const cards = o.pairing === "front_back" ? Math.ceil(images / 2) : images;
  const identifies = o.task === "identify";
  const byXimilar = identifies && o.engine === "ximilar";
  const crops = cropsOf(o) ? cards : 0;
  const graded = gradingOf(o) !== "off" && gradeEngineOf(o) === "ximilar" ? cards : 0;
  const sync = identifies && o.syncRemote ? cards + 1 : 0;
  const calls = (byXimilar ? cards : 0) + graded + crops + sync;
  if (!calls) {
    if (!identifies) return t("newSession.cost.localGrade");
    return t(o.engine === "agent" ? "newSession.cost.agent" : "newSession.cost.local");
  }
  if (identifies && o.engine === "local" && calls === sync) return t("newSession.cost.callsCloud", { count: calls });
  return t(byXimilar && o.magicAi ? "newSession.cost.callsMagicAi" : "newSession.cost.calls", { count: calls });
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
  const t = useT();
  return (
    <div className="col-span-2 space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
      <div className="space-y-1">
        <Check
          label={t("newSession.bothSides.label")}
          checked={options.bothSides}
          // The back has to be paired with its front first.
          onChange={(v) => onChange({ bothSides: v, ...(v && !csv ? { pairing: "front_back" } : {}) })}
        />
        <p className="pl-6 text-xs text-slate-500">
          {csv
            ? t("newSession.bothSides.csv")
            : rich(t("newSession.bothSides.paired"), { code: (s) => <code>{s}</code> })}{" "}
          {t("newSession.bothSides.why")}
          {options.bothSides && !readsBacks && (
            <span className="text-amber-700 dark:text-amber-400">
              {" "}
              {t("newSession.bothSides.frontOnly", { type: idModeLabels()[idMode] })}
            </span>
          )}
        </p>
      </div>
      <div className="space-y-1">
        <Check
          label={t("newSession.bothSides.magicAi")}
          checked={options.magicAi}
          onChange={(v) => onChange({ magicAi: v })}
        />
        <p className="pl-6 text-xs text-slate-500">
          {t("newSession.bothSides.magicAiHint")}
          {options.magicAi && !sport && (
            <span className="text-amber-700 dark:text-amber-400"> {t("newSession.bothSides.magicAiSportOnly")}</span>
          )}
        </p>
      </div>
      {idMode === "auto" && !custom && (options.bothSides || options.magicAi) && (
        <p className="text-xs text-slate-500">{t("newSession.bothSides.autoNote")}</p>
      )}
      <div>
        <label className="label">{t("newSession.bothSides.customEndpoint")}</label>
        <input
          className="input font-mono text-xs"
          value={options.customEndpoint}
          onChange={(e) => onChange({ customEndpoint: e.target.value })}
          placeholder="collectibles/v2/…"
          spellCheck={false}
        />
        <p className="mt-1 text-xs text-slate-500">
          {idMode === "auto"
            ? t("newSession.bothSides.customEndpointHintAuto")
            : t("newSession.bothSides.customEndpointHint", { type: idModeLabels()[idMode] })}
        </p>
      </div>
    </div>
  );
}

const MISSING = "border-amber-400 ring-2 ring-amber-400/25 dark:border-amber-500";

// Functions, not constants: the labels follow the app's language.

const TASK_ICONS: Record<TaskChoice, typeof Search> = {
  identify: Search,
  prices: DollarSign,
  grade: BadgeCheck,
  crop: Crop,
};

/** Where prices come from; Ximilar's come only with its identification. */
export function priceOptions(ximilarIdentifies: boolean): SelectOption<Prices>[] {
  const icons = { ximilar: Cloud, ebay: ShoppingBag, agent: Bot };
  return (["ximilar", "ebay", "agent"] as const).map((value) => ({
    value,
    label: t(`newSession.priceEngines.${value}`),
    hint: t(`newSession.priceHints.${value}`),
    icon: icons[value],
    disabled: value === "ximilar" && !ximilarIdentifies,
  }));
}

/** What the Grade tab grades: a full grade, centering only, or the condition. */
function gradeModeOptions(): SelectOption<Exclude<GradingMode, "off">>[] {
  const labels = gradingLabels();
  return (["grade", "centering", "condition"] as const).map((value) => ({ value, label: labels[value] }));
}

export function taskOptions(): SelectOption<Task>[] {
  const labels = taskLabels();
  const icons = { identify: Search, grade: BadgeCheck, condition: Gauge, crop: Crop };
  return (["identify", "grade", "condition", "crop"] as const).map((value) => ({
    value,
    label: labels[value],
    hint: t(`status.taskHint.${value}`),
    icon: icons[value],
  }));
}

export function cropFormatOptions(): SelectOption<CropFormat>[] {
  return [
    { value: "jpg", label: "JPEG", hint: t("newSession.crop.formats.jpg") },
    { value: "png", label: "PNG", hint: t("newSession.crop.formats.png") },
    { value: "webp", label: "WebP", hint: t("newSession.crop.formats.webp") },
  ];
}

/** `identifies`: an AI agent only identifies, so it is offered for that task alone. */
export function engineOptions(identifies = true): SelectOption<Engine>[] {
  const labels = engineLabels();
  return [
    { value: "ximilar", label: labels.ximilar, icon: Cloud },
    { value: "local", label: labels.local, icon: Cpu },
    ...(identifies
      ? [{ value: "agent" as const, label: labels.agent, icon: Bot, hint: t("newSession.agentHint") }]
      : []),
  ];
}

export function dpiOptions(): SelectOption<string>[] {
  return [
    { value: "300", label: "300 dpi", hint: t("newSession.options.dpiIdentification") },
    { value: "600", label: "600 dpi", hint: t("newSession.options.dpiGrading") },
  ];
}

/** Condition scales; the local model knows all but Ximilar's own (`local::grade::SCALES`). */
export function conditionOptions(): SelectOption<string>[] {
  return [
    { value: "tcgplayer", label: "TCGplayer" },
    { value: "cardmarket", label: "Cardmarket" },
    { value: "ebay_tcg", label: "eBay", hint: t("newSession.options.tradingCards") },
    { value: "ebay_sport", label: "eBay", hint: t("newSession.options.sportsCards") },
    { value: "ximilar", label: "Ximilar", hint: t("newSession.options.apiOnly") },
  ];
}
export function localConditionOptions(): SelectOption<string>[] {
  return conditionOptions().filter((o) => o.value !== "ximilar");
}

const CONDITION_SCALES = ["tcgplayer", "cardmarket", "ebay_tcg", "ebay_sport", "ximilar"];

/** The scale shown for a session's mode, as the backend reads it (`local::grade::scale`). */
export function conditionScale(mode: string, local: boolean): string {
  if (mode === "ebay") return local ? "ebay_sport" : "ebay_tcg";
  if (local && (mode === "ximilar" || !CONDITION_SCALES.includes(mode))) return "tcgplayer";
  return CONDITION_SCALES.includes(mode) ? mode : "ebay_tcg";
}

export function multifeedOptions(): SelectOption<"off" | "default">[] {
  return [
    { value: "off", label: t("common.off"), hint: t("newSession.options.multifeedOffHint") },
    { value: "default", label: t("newSession.options.multifeedDefault") },
  ];
}

export function pairingOptions(): SelectOption<Pairing>[] {
  return [
    { value: "single", label: t("newSession.options.single") },
    { value: "front_back", label: t("newSession.options.frontBack") },
  ];
}

/** Opens Settings at one of its tabs. */
export function OpenSettings({ section, onOpen }: { section: string; onOpen: () => void }) {
  const navigate = useNavigate();
  const t = useT();
  return (
    <button
      className="btn shrink-0 py-1 text-xs"
      onClick={() => {
        onOpen();
        navigate("/settings", { state: { section } });
      }}
    >
      <Settings className="h-3.5 w-3.5" /> {t("newSession.openSettings")}
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
  const t = useT();
  const id = value.match(UUID)?.pop()?.toLowerCase() ?? null;
  const [found, setFound] = useState<{ id: string; text: string; ok: boolean } | null>(null);

  useEffect(() => {
    if (!id) return;
    let alive = true;
    const timer = setTimeout(() => {
      api
        .getRemoteSession(id)
        .then(({ session: r }) => {
          if (!alive) return;
          const name = r.name || t("newSession.remote.untitled");
          setFound({ id, ok: true, text: t("newSession.remote.found", { name, count: r.identification_count }) });
          if (r.name) onFound(r.name);
        })
        .catch((e) => alive && setFound({ id, ok: false, text: errorText(e) }));
    }, 300);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
    // onFound is a new function each render; the lookup depends on the ID only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const shown = found?.id === id ? found : null;
  return (
    <div className="col-span-2">
      <label className="label">{t("newSession.remote.label")}</label>
      <input
        className="input font-mono text-xs"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={t("newSession.remote.placeholder")}
        spellCheck={false}
      />
      <p className={clsx("mt-1 text-xs", shown && !shown.ok ? "text-red-600" : "text-slate-500")}>
        {value.trim() && !id
          ? t("newSession.remote.paste")
          : shown
            ? shown.ok
              ? t(folder ? "newSession.remote.addedSkips" : "newSession.remote.added", { found: shown.text })
              : shown.text
            : id
              ? t("newSession.remote.lookingUp")
              : t(folder ? "newSession.remote.hintSkips" : "newSession.remote.hint")}
      </p>
    </div>
  );
}

function CsvSummary({ preview: p }: { preview: CsvPreview }) {
  const t = useT();
  const counts = {
    rows: t("newSession.csv.rows", { count: p.rows }),
    cards: t("newSession.csv.identifiedCards", { count: p.cards }),
    images: t("newSession.csv.images", { count: p.images }),
    skipped: p.skipped,
  };
  return (
    <div className="rounded-md bg-slate-50 px-3 py-2 text-xs text-slate-600 dark:bg-slate-900 dark:text-slate-300">
      <p>
        {rich(t(p.skipped > 0 ? "newSession.csv.summarySkipped" : "newSession.csv.summary", counts), {
          b: (s) => <b>{s}</b>,
        })}
        {p.missingImages > 0 && (
          <span className="text-amber-700 dark:text-amber-400">
            {" "}
            {t("newSession.csv.missingImages", { count: p.missingImages })}
          </span>
        )}
      </p>
      <p className="mt-1 text-slate-500">{t("newSession.csv.columns", { columns: p.columns.join(", ") })}</p>
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
