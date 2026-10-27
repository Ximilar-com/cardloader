import { open } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import {
  Bot,
  CheckCircle2,
  Copy,
  Cloud,
  Cpu,
  FolderOpen,
  Info,
  KeyRound,
  Languages,
  LogIn,
  LogOut,
  MapPin,
  Plus,
  RefreshCw,
  ScanLine,
  SlidersHorizontal,
  Store,
  Wand2,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useLocation } from "react-router-dom";
import { create } from "zustand";
import { ebayReady } from "../components/Ebay";
import { ModelPicker, useModels } from "../components/LocalModels";
import { ErrorNote } from "../components/Modal";
import { toast } from "../components/Toast";
import {
  Check,
  conditionOptions,
  conditionScale,
  dpiOptions,
  gameOptions,
  multifeedOptions,
  pairingOptions,
} from "../components/NewSessionDialog";
import { useSetupWizard } from "../components/SetupWizard";
import Select, { optionsOf, type SelectOption } from "../components/Select";
import { languageOf, rich, t, useI18n, useT, type Language } from "../i18n";
import { LANGUAGES } from "../i18n/languages";
import {
  api,
  errorText,
  type AppInfo,
  type ConnectionCheck,
  type DeviceInfo,
  type EbaySettings,
  type EbaySetup,
  type McpSettings,
  type McpStatus,
  type ScannerSupport,
  type Settings,
  type SettingsView,
} from "../lib/api";
import { bytes, dateTime, gradingLabels, idModeLabels, sourceLabels } from "../lib/format";

const DEFAULT_API = "https://api.ximilar.com";

/** Settings changed but not saved yet: kept when the user leaves the page, so nothing typed is lost. */
/** The tab to show when the page remounts after a language change. */
let reopenTab: Tab | null = null;

export const useUnsavedSettings = create<{
  saved: Settings | null;
  draft: Settings | null;
  setDraft: (d: Settings | ((d: Settings | null) => Settings | null)) => void;
}>((set) => ({
  saved: null,
  draft: null,
  setDraft: (d) => set((s) => ({ draft: typeof d === "function" ? d(s.draft) : d })),
}));

export function isUnsaved(saved: Settings | null, draft: Settings | null): boolean {
  return !!saved && !!draft && JSON.stringify(strip(saved)) !== JSON.stringify(strip(draft));
}

/** What is wrong with the settings, by field; empty when they can be saved. */
function problems(s: Settings): Partial<Record<"apiBaseUrl" | "page" | "port", string>> {
  const out: Partial<Record<"apiBaseUrl" | "page" | "port", string>> = {};
  if (!/^https?:\/\/[^\s/]+/.test(s.apiBaseUrl.trim())) out.apiBaseUrl = t("settings.problems.apiBaseUrl");
  const mm = [s.scan.pageWidthMm, s.scan.pageHeightMm];
  if (mm.some((v) => !Number.isFinite(v) || v < 20 || v > 400)) out.page = t("settings.problems.page");
  if (!Number.isInteger(s.mcp.port) || s.mcp.port < 1024 || s.mcp.port > 65535) out.port = t("settings.problems.port");
  return out;
}

export default function SettingsPage() {
  const t = useT();
  const [view, setViewState] = useState<SettingsView | null>(null);
  const draft = useUnsavedSettings((s) => s.draft);
  const setDraft = useUnsavedSettings((s) => s.setDraft);
  const setView = (v: SettingsView) => {
    setViewState(v);
    useUnsavedSettings.setState({ saved: strip(v) });
  };
  const [token, setToken] = useState("");
  const [check, setCheck] = useState<ConnectionCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [support, setSupport] = useState<ScannerSupport | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[] | null>(null);
  const [findingScanners, setFindingScanners] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [mcp, setMcp] = useState<McpStatus | null>(null);
  const models = useModels();
  const showSetup = useSetupWizard((s) => s.show);
  // Another page can open Settings at a tab: navigate("/settings", { state: { section } }).
  const section = (useLocation().state as { section?: Tab } | null)?.section;
  const [tab, setTab] = useState<Tab>(() => reopenTab ?? section ?? "ximilar");
  useEffect(() => {
    reopenTab = null;
  }, []);
  const tokenInput = useRef<HTMLInputElement>(null);
  const loaded = !!view;

  useEffect(() => {
    if (loaded && section === "ximilar") tokenInput.current?.focus();
  }, [loaded, section]);

  useEffect(() => {
    api.getSettings().then((v) => {
      // Edits left unsaved on an earlier visit stay; otherwise start from what is stored.
      const { saved, draft } = useUnsavedSettings.getState();
      setViewState(v);
      useUnsavedSettings.setState({
        saved: strip(v),
        draft: isUnsaved(saved, draft) ? { ...draft!, ebay: { ...draft!.ebay, username: v.ebay.username } } : strip(v),
      });
      // Show at once whether the saved token still works.
      if (v.tokenSet) {
        setChecking(true);
        api
          .testConnection()
          .then(setCheck)
          .catch(() => {})
          .finally(() => setChecking(false));
      }
    });
    api.scannerSupport().then(setSupport);
    api.mcpStatus().then(setMcp);
  }, []);

  if (!view || !draft) return null;
  const dirty = isUnsaved(view, draft);
  const invalid = problems(draft);
  const valid = Object.keys(invalid).length === 0;
  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setSaved(false);
    setDraft((d) => d && { ...d, [key]: value });
  };
  const setEbay = (patch: Partial<EbaySettings>) => {
    setSaved(false);
    setDraft((d) => d && { ...d, ebay: { ...d.ebay, ...patch } });
  };
  // The eBay connection changes on its own (not with Save): take the new state, keep the other edits.
  const applyView = (v: SettingsView) => {
    setView(v);
    setDraft((d) => d && { ...d, ebay: { ...d.ebay, username: v.ebay.username } });
  };
  const setMcpSettings = (patch: Partial<McpSettings>) => set("mcp", { ...draft.mcp, ...patch });
  const setScan = <K extends keyof Settings["scan"]>(key: K, value: Settings["scan"][K]) =>
    set("scan", { ...draft.scan, [key]: value });

  const save = async () => {
    setError(null);
    try {
      const v = await api.saveSettings({ ...draft, apiBaseUrl: draft.apiBaseUrl.trim() });
      setView(v);
      setDraft(strip(v));
      setSaved(true);
      // Saving starts or stops the agents' server.
      api.mcpStatus().then(setMcp);
      if (languageOf(v.language) !== useI18n.getState().language) {
        // The app remounts its pages in the new language: come back to this tab.
        reopenTab = tab;
        useI18n.getState().setLanguage(v.language);
      }
    } catch (e) {
      setError(errorText(e));
    }
  };

  const saveToken = async (value: string) => {
    setError(null);
    setCheck(null);
    try {
      const v = await api.setToken(value);
      setView({ ...view, tokenSet: v.tokenSet, tokenStorage: v.tokenStorage });
      setToken("");
      if (v.tokenSet) testConnection();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const testConnection = () => {
    setChecking(true);
    api
      .testConnection()
      .then(setCheck)
      .catch((e) =>
        setCheck({ ok: false, account: null, error: errorText(e), cardsCloud: false, cardsCloudError: null }),
      )
      .finally(() => setChecking(false));
  };

  const findScanners = () => {
    setFindingScanners(true);
    api
      .listScanners()
      .then(setDevices)
      .catch((e) => setError(errorText(e)))
      .finally(() => setFindingScanners(false));
  };

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-4 border-b border-slate-200 px-6 py-4 dark:border-slate-800">
        <h1 className="text-lg font-semibold">{t("settings.title")}</h1>
        <div className="flex-1" />
        {saved && !dirty && <span className="text-sm text-emerald-600">{t("common.saved")}</span>}
        {dirty && (
          <span className="flex items-center gap-1.5 text-sm font-medium text-amber-700 dark:text-amber-400">
            <span className="h-2 w-2 animate-pulse rounded-full bg-amber-500" /> {t("settings.unsavedChanges")}
          </span>
        )}
        <button className="btn" disabled={!dirty} onClick={() => setDraft(strip(view))}>
          {t("settings.discard")}
        </button>
        {/* Gray until something changes, then the accent with a ring, so a pending save stands out. */}
        <button
          className={clsx("btn", dirty && valid && "btn-primary ring-4 ring-accent/25")}
          disabled={!dirty || !valid}
          onClick={save}
          title={valid ? undefined : t("settings.fixFirst")}
        >
          {t("common.save")}
        </button>
      </header>
      <div className="flex min-h-0 flex-1">
        <nav
          className="w-52 shrink-0 space-y-0.5 border-r border-slate-200 p-3 dark:border-slate-800"
          aria-label={t("settings.title")}
        >
          {tabs().map(({ key, label, icon: Icon }) => (
            <button
              key={key}
              className={clsx(
                "flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-sm",
                tab === key
                  ? "bg-accent/10 font-medium text-accent"
                  : "text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800",
              )}
              aria-current={tab === key ? "page" : undefined}
              onClick={() => setTab(key)}
            >
              <Icon className="h-4 w-4 shrink-0" />
              <span className="flex-1">{label}</span>
              {key === "ximilar" && view.tokenSet && (
                <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" aria-label={t("settings.tokenSaved")} />
              )}
              {key === "ebay" && view.ebayConnected && ebayReady(view.ebay) && (
                <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" aria-label={t("settings.ebayConnected")} />
              )}
              {key === "agents" && mcp?.running && (
                <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" aria-label={t("settings.agentsCanConnect")} />
              )}
            </button>
          ))}
        </nav>
        <div className="min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto max-w-3xl space-y-6 px-6 py-6">
            {error && <ErrorNote>{error}</ErrorNote>}

            {tab === "ximilar" && (
              <>
                <Section
                  title={t("settings.ximilar.title")}
                  done={view.tokenSet}
                  text={rich(t("settings.ximilar.text"), {
                    link: (s) => (
                      <button className="text-accent underline" onClick={() => openUrl("https://app.ximilar.com")}>
                        {s}
                      </button>
                    ),
                  })}
                >
                  <div>
                    <label className="label">{t("settings.ximilar.token")}</label>
                    <div className="flex gap-2">
                      <input
                        ref={tokenInput}
                        type="password"
                        className="input font-mono"
                        placeholder={
                          view.tokenSet ? t("settings.savedPlaceholder") : t("settings.ximilar.tokenPlaceholder")
                        }
                        value={token}
                        onChange={(e) => setToken(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && token && saveToken(token)}
                      />
                      <button className="btn btn-primary" disabled={!token} onClick={() => saveToken(token)}>
                        <KeyRound className="h-4 w-4" /> {t("settings.ximilar.saveToken")}
                      </button>
                      {view.tokenSet && (
                        <button className="btn" onClick={() => saveToken("")}>
                          {t("common.remove")}
                        </button>
                      )}
                    </div>
                    <p className="mt-1 text-xs text-slate-500">
                      {view.tokenSet
                        ? view.tokenStorage === "keychain"
                          ? t("settings.ximilar.storedKeychain")
                          : view.tokenStorage === "dev"
                            ? t("settings.ximilar.storedDev")
                            : t("settings.ximilar.storedFile")
                        : t("settings.ximilar.noToken")}
                    </p>
                  </div>
                  <div className="flex items-center gap-3">
                    <button className="btn" disabled={!view.tokenSet || checking} onClick={testConnection}>
                      <RefreshCw className={clsx("h-4 w-4", checking && "animate-spin")} />{" "}
                      {t("settings.ximilar.testConnection")}
                    </button>
                    {check && (
                      <div className="space-y-0.5 text-sm">
                        <Status
                          ok={check.ok}
                          text={
                            check.ok
                              ? check.account
                                ? t("settings.connectedAs", { account: check.account })
                                : t("settings.connected")
                              : (check.error ?? t("settings.ximilar.failed"))
                          }
                        />
                        <Status
                          ok={check.cardsCloud}
                          text={
                            check.cardsCloud
                              ? t("settings.ximilar.cardsCloudAvailable")
                              : check.cardsCloudError
                                ? t("settings.ximilar.cardsCloudUnavailableWhy", { error: check.cardsCloudError })
                                : t("settings.ximilar.cardsCloudUnavailable")
                          }
                        />
                      </div>
                    )}
                  </div>
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <label className="label">{t("settings.ximilar.workspace")}</label>
                      <input
                        className="input font-mono"
                        placeholder={t("settings.ximilar.workspacePlaceholder")}
                        value={draft.workspace ?? ""}
                        onChange={(e) => set("workspace", e.target.value || null)}
                      />
                    </div>
                    <div>
                      <label className="label flex items-center justify-between">
                        {t("settings.ximilar.baseUrl")}
                        {draft.apiBaseUrl.trim() !== DEFAULT_API && (
                          <button
                            className="text-xs font-normal text-accent"
                            onClick={() => set("apiBaseUrl", DEFAULT_API)}
                          >
                            {t("settings.ximilar.reset")}
                          </button>
                        )}
                      </label>
                      <input
                        className={clsx("input font-mono", invalid.apiBaseUrl && "border-red-400")}
                        value={draft.apiBaseUrl}
                        onChange={(e) => set("apiBaseUrl", e.target.value)}
                      />
                      {invalid.apiBaseUrl && <p className="mt-1 text-xs text-red-600">{invalid.apiBaseUrl}</p>}
                    </div>
                  </div>
                </Section>
              </>
            )}
            {tab === "models" && (
              <Section
                title={t("settings.models.title")}
                text={rich(t("settings.models.text"), { b: (s) => <b>{s}</b> })}
              >
                {models && (
                  <ModelPicker models={models} value={draft.localModel} onChange={(id) => set("localModel", id)} />
                )}
              </Section>
            )}
            {tab === "sessions" && (
              <>
                <Section title={t("settings.sessions.identification")} text={t("settings.sessions.identificationText")}>
                  <div className="grid grid-cols-2 gap-3">
                    <EngineOption
                      active={draft.engine === "ximilar"}
                      onClick={() => set("engine", "ximilar")}
                      icon={<Cloud className="h-5 w-5" />}
                      title={t("settings.sessions.ximilarTitle")}
                      badge={t("settings.sessions.ximilarBadge")}
                      text={t("settings.sessions.ximilarText")}
                    />
                    <EngineOption
                      active={draft.engine === "local"}
                      onClick={() => set("engine", "local")}
                      icon={<Cpu className="h-5 w-5" />}
                      title={t("settings.sessions.localTitle")}
                      text={t("settings.sessions.localText")}
                    />
                  </div>
                  {draft.engine === "local" && (
                    <p className="text-sm text-slate-500">
                      {rich(
                        t("settings.sessions.localUses", {
                          model: models?.find((m) => m.id === draft.localModel)?.name ?? draft.localModel,
                        }),
                        {
                          link: (s) => (
                            <button className="text-accent hover:underline" onClick={() => setTab("models")}>
                              {s}
                            </button>
                          ),
                        },
                      )}
                    </p>
                  )}
                </Section>
                <Section title={t("settings.sessions.title")} text={t("settings.sessions.text")}>
                  <div className="grid grid-cols-2 gap-4">
                    <div className="col-span-2">
                      <label className="label">{t("settings.sessions.defaultFolder")}</label>
                      <div className="flex gap-2">
                        <input
                          className="input"
                          value={draft.defaultFolder ?? ""}
                          onChange={(e) => set("defaultFolder", e.target.value || null)}
                        />
                        <button
                          className="btn"
                          onClick={async () => {
                            const picked = await open({
                              directory: true,
                              defaultPath: draft.defaultFolder ?? undefined,
                            });
                            if (typeof picked === "string") set("defaultFolder", picked);
                          }}
                        >
                          <FolderOpen className="h-4 w-4" /> {t("settings.sessions.browse")}
                        </button>
                      </div>
                    </div>
                    <Field
                      label={t("settings.sessions.identifyAs")}
                      value={draft.defaultIdMode}
                      options={optionsOf(idModeLabels())}
                      onChange={(v) => set("defaultIdMode", v)}
                    />
                    {draft.defaultIdMode === "tcg" ? (
                      <Field
                        label={t("settings.sessions.game")}
                        value={draft.defaultSubcategory}
                        options={gameOptions()}
                        onChange={(v) => set("defaultSubcategory", v)}
                      />
                    ) : (
                      <div />
                    )}
                    <Field
                      label={t("settings.sessions.opens")}
                      value={draft.defaultSources[0] ?? "folder"}
                      options={sourceOptions()}
                      onChange={(v) => set("defaultSources", [v, ...draft.defaultSources.filter((s) => s !== v)])}
                    />
                    <Field
                      label={t("settings.sessions.grading")}
                      value={draft.defaultGrading}
                      options={optionsOf(gradingLabels())}
                      onChange={(v) => set("defaultGrading", v)}
                    />
                    <Field
                      label={t("settings.sessions.conditionScale")}
                      value={conditionScale(draft.defaultConditionMode, false)}
                      options={conditionOptions()}
                      onChange={(v) => set("defaultConditionMode", v)}
                    />
                    <Field
                      label={t("settings.sessions.imagesPerCard")}
                      value={draft.defaultPairing}
                      options={pairingOptions()}
                      onChange={(v) => set("defaultPairing", v)}
                    />
                    <Field
                      label={t("settings.sessions.parallel")}
                      value={String(Math.min(draft.concurrency, 3))}
                      options={concurrencyOptions()}
                      onChange={(v) => set("concurrency", Number(v))}
                    />
                    <Check
                      label={t("settings.sessions.priceStats")}
                      checked={draft.priceStats}
                      onChange={(v) => set("priceStats", v)}
                    />
                    <Check
                      label={t("settings.sessions.syncByDefault")}
                      checked={draft.syncByDefault}
                      onChange={(v) => set("syncByDefault", v)}
                    />
                  </div>
                  <button className="btn mt-4" onClick={() => showSetup(view)}>
                    <Wand2 className="h-4 w-4" /> {t("settings.sessions.runSetup")}
                  </button>
                </Section>
              </>
            )}
            {tab === "ebay" && (
              <EbayTab view={view} draft={draft} setEbay={setEbay} applyView={applyView} setError={setError} />
            )}
            {tab === "scanner" && (
              <Section
                title={t("settings.scanner.title")}
                text={support?.backend === "twain" ? t("settings.scanner.twainText") : t("settings.scanner.saneText")}
              >
                {support && !support.available && <ErrorNote>{support.hint}</ErrorNote>}
                <div className="grid grid-cols-2 gap-4">
                  <div className="col-span-2">
                    <label className="label">{t("settings.scanner.defaultScanner")}</label>
                    <div className="flex gap-2">
                      <Select
                        value={draft.defaultScanner ?? ""}
                        onChange={(v) => set("defaultScanner", v || null)}
                        aria-label={t("settings.scanner.defaultScanner")}
                        options={[
                          { value: "", label: t("common.none") },
                          ...(draft.defaultScanner && !devices?.some((d) => d.id === draft.defaultScanner)
                            ? [{ value: draft.defaultScanner, label: draft.defaultScanner, icon: ScanLine }]
                            : []),
                          ...(devices ?? []).map((d) => ({ value: d.id, label: d.name || d.id, icon: ScanLine })),
                        ]}
                      />
                      <button className="btn" onClick={findScanners} disabled={findingScanners || !support?.available}>
                        <RefreshCw className={clsx("h-4 w-4", findingScanners && "animate-spin")} />{" "}
                        {t("settings.scanner.findScanners")}
                      </button>
                    </div>
                    {support?.backend === "twain" && (
                      <DriverSettings device={draft.defaultScanner} disabled={!support.available} />
                    )}
                  </div>
                  <Field
                    label={t("settings.scanner.resolution")}
                    value={String(draft.scan.dpi)}
                    options={dpiOptions()}
                    onChange={(v) => setScan("dpi", Number(v))}
                  />
                  <Field
                    label={t("settings.scanner.colour")}
                    value={draft.scan.color}
                    options={colorOptions()}
                    onChange={(v) => setScan("color", v)}
                  />
                  <div>
                    <label className="label">{t("settings.scanner.pageWidth")}</label>
                    <input
                      type="number"
                      className={clsx("input", invalid.page && "border-red-400")}
                      value={draft.scan.pageWidthMm}
                      onChange={(e) => setScan("pageWidthMm", Number(e.target.value))}
                    />
                  </div>
                  <div>
                    <label className="label flex items-center justify-between">
                      {t("settings.scanner.pageHeight")}
                      {(draft.scan.pageWidthMm !== 66 || draft.scan.pageHeightMm !== 92) && (
                        <button
                          className="text-xs font-normal text-accent"
                          onClick={() => set("scan", { ...draft.scan, pageWidthMm: 66, pageHeightMm: 92 })}
                        >
                          {t("settings.scanner.resetPage")}
                        </button>
                      )}
                    </label>
                    <input
                      type="number"
                      className={clsx("input", invalid.page && "border-red-400")}
                      value={draft.scan.pageHeightMm}
                      onChange={(e) => setScan("pageHeightMm", Number(e.target.value))}
                    />
                    {invalid.page && <p className="mt-1 text-xs text-red-600">{invalid.page}</p>}
                  </div>
                  <Field
                    label={t("settings.scanner.multifeed")}
                    value={draft.scan.multifeed}
                    options={multifeedOptions()}
                    onChange={(v) => setScan("multifeed", v)}
                  />
                  <Field
                    label={t("settings.scanner.rotation")}
                    value={draft.scan.rotation}
                    options={rotationOptions()}
                    onChange={(v) => setScan("rotation", v)}
                  />
                  <div className="flex items-end pb-2">
                    <Check
                      label={t("settings.scanner.duplex")}
                      checked={draft.scan.duplex}
                      onChange={(v) => setScan("duplex", v)}
                    />
                  </div>
                  {draft.scan.duplex && (
                    <Field
                      label={t("settings.scanner.sideOrder")}
                      value={draft.scan.backFirst ? "back" : "front"}
                      options={sideOrderOptions()}
                      onChange={(v) => setScan("backFirst", v === "back")}
                    />
                  )}
                </div>
                <p className="text-xs text-slate-500">{t("settings.scanner.note")}</p>
              </Section>
            )}
            {tab === "agents" && (
              <AgentsTab
                draft={draft.mcp}
                set={setMcpSettings}
                status={mcp}
                setStatus={setMcp}
                portProblem={invalid.port}
                setError={setError}
              />
            )}
            {tab === "language" && (
              <Section title={t("common.language")} text={t("settings.language.text")}>
                <Select<Language>
                  value={languageOf(draft.language)}
                  onChange={(v) => set("language", v)}
                  aria-label={t("common.language")}
                  options={LANGUAGES.map(({ code, name, english, Flag }) => ({
                    value: code,
                    label: (
                      <span className="flex items-center gap-2">
                        <Flag className="h-3.5 w-5 rounded-[2px] ring-1 ring-black/10" />
                        {name}
                      </span>
                    ),
                    text: name,
                    hint: english !== name ? english : undefined,
                  }))}
                />
              </Section>
            )}
            {tab === "about" && <About />}
          </div>
        </div>
      </div>
    </div>
  );
}

/** The TWAIN driver's own dialog: automatic cropping, deskew and the like, kept for every later scan. */
function permissions(): { key: keyof McpSettings; label: string; hint: string }[] {
  return [
    { key: "allowRun", label: t("settings.agents.permissions.run"), hint: t("settings.agents.permissions.runHint") },
    {
      key: "allowDelete",
      label: t("settings.agents.permissions.delete"),
      hint: t("settings.agents.permissions.deleteHint"),
    },
    {
      key: "allowMarket",
      label: t("settings.agents.permissions.market"),
      hint: t("settings.agents.permissions.marketHint"),
    },
    {
      key: "allowSettings",
      label: t("settings.agents.permissions.settings"),
      hint: t("settings.agents.permissions.settingsHint"),
    },
  ];
}

/** AI agents (Claude Desktop, Claude Code, Cursor, …) working with Cardloader over MCP. */
function AgentsTab({
  draft,
  set,
  status,
  setStatus,
  portProblem,
  setError,
}: {
  draft: McpSettings;
  set: (patch: Partial<McpSettings>) => void;
  status: McpStatus | null;
  setStatus: (s: McpStatus) => void;
  portProblem?: string;
  setError: (e: string | null) => void;
}) {
  const t = useT();
  const command = status?.command ?? "";
  const [app, setApp] = useState<AgentApp>("claude");
  // Claude Desktop, Cursor, Gemini CLI and LM Studio read the same `mcpServers` shape.
  const mcpServers = JSON.stringify({ mcpServers: { cardloader: { command, args: ["mcp"] } } }, null, 2);
  const newToken = () =>
    api
      .regenerateMcpToken()
      .then((s) => {
        setStatus(s);
        toast(t("settings.agents.newTokenToast"));
      })
      .catch((e) => setError(errorText(e)));
  return (
    <>
      <Section title={t("settings.agents.title")} done={status?.running} text={t("settings.agents.text")}>
        <Check label={t("settings.agents.enable")} checked={draft.enabled} onChange={(v) => set({ enabled: v })} />
        <div className="space-y-2">
          <div className="label">{t("settings.agents.may")}</div>
          <Check label={t("settings.agents.read")} checked disabled onChange={() => {}} />
          {permissions().map((p) => (
            <div key={p.key}>
              <Check
                label={p.label}
                checked={draft[p.key] as boolean}
                disabled={!draft.enabled}
                onChange={(v) => set({ [p.key]: v })}
              />
              <p className="ml-6 text-xs text-slate-500">{p.hint}</p>
            </div>
          ))}
        </div>
        <div className="flex items-end gap-3">
          <div>
            <div className="label">{t("settings.agents.port")}</div>
            <input
              className="input w-28"
              type="number"
              value={draft.port}
              disabled={!draft.enabled}
              onChange={(e) => set({ port: Number(e.target.value) })}
            />
          </div>
          <p className="pb-2 text-xs text-slate-500">{portProblem ?? t("settings.agents.portHint")}</p>
        </div>
        {status?.error ? (
          <Status ok={false} text={status.error} />
        ) : status?.running ? (
          <Status
            ok
            text={
              status.lastUsed
                ? t("settings.agents.runningUsed", { url: status.url ?? "", time: dateTime(status.lastUsed) })
                : t("settings.agents.running", { url: status.url ?? "" })
            }
          />
        ) : (
          draft.enabled && <p className="text-sm text-slate-500">{t("settings.agents.saveToStart")}</p>
        )}
      </Section>
      {status?.running && (
        <Section title={t("settings.agents.connect.title")} text={t("settings.agents.connect.text")}>
          <div className="flex flex-wrap gap-0.5 rounded-md bg-slate-100 p-0.5 dark:bg-slate-900" role="tablist">
            {agentApps().map(([key, label]) => (
              <button
                key={key}
                role="tab"
                aria-selected={app === key}
                onClick={() => setApp(key)}
                className={clsx(
                  "rounded px-3 py-1 text-sm font-medium",
                  app === key
                    ? "bg-white shadow-xs dark:bg-slate-800"
                    : "text-slate-500 hover:text-slate-800 dark:hover:text-slate-200",
                )}
              >
                {label}
              </button>
            ))}
          </div>
          {/* Its own spacing: snippets of one app sit apart from each other and from the tips below. */}
          <div role="tabpanel" className="space-y-5 pb-1">
            {app === "claude" && (
              <>
                <Snippet
                  label="Claude Desktop"
                  hint={t("settings.agents.connect.claudeDesktopHint")}
                  text={mcpServers}
                />
                <Snippet
                  label="Claude Code"
                  hint={t("settings.agents.connect.claudeCodeHint")}
                  text={`claude mcp add cardloader -- "${command}" mcp`}
                />
              </>
            )}
            {app === "codex" && (
              <>
                <Snippet
                  label={t("settings.agents.connect.codex")}
                  hint={t("settings.agents.connect.codexHint")}
                  text={`codex mcp add cardloader -- "${command}" mcp`}
                />
                <Snippet
                  label={t("settings.agents.connect.configFile")}
                  hint={t("settings.agents.connect.codexConfigHint")}
                  text={`[mcp_servers.cardloader]\ncommand = '${command}'\nargs = ["mcp"]`}
                />
              </>
            )}
            {app === "cursor" && (
              <Snippet label="Cursor" hint={t("settings.agents.connect.cursorHint")} text={mcpServers} />
            )}
            {app === "gemini" && (
              <Snippet label="Gemini CLI" hint={t("settings.agents.connect.geminiHint")} text={mcpServers} />
            )}
            {app === "lmstudio" && (
              <Snippet label="LM Studio" hint={t("settings.agents.connect.lmstudioHint")} text={mcpServers} />
            )}
            {app === "http" && (
              <div className="space-y-2">
                <Snippet
                  label={t("settings.agents.connect.address")}
                  hint={t("settings.agents.connect.addressHint")}
                  text={status.url ?? ""}
                />
                <Snippet
                  label={t("settings.agents.connect.header")}
                  text={`Authorization: Bearer ${status.token ?? ""}`}
                />
                <button className="btn px-2 py-1 text-xs" onClick={newToken}>
                  <RefreshCw className="h-3.5 w-3.5" /> {t("settings.agents.connect.newToken")}
                </button>
              </div>
            )}
          </div>
          <p className="text-xs text-slate-500">{t("settings.agents.connect.tryAsking")}</p>
        </Section>
      )}
    </>
  );
}

type AgentApp = "claude" | "codex" | "cursor" | "gemini" | "lmstudio" | "http";

function agentApps(): [AgentApp, string][] {
  return [
    ["claude", "Claude"],
    ["codex", "OpenAI Codex"],
    ["cursor", "Cursor"],
    ["gemini", "Gemini CLI"],
    ["lmstudio", "LM Studio"],
    ["http", t("settings.agents.connect.otherApp")],
  ];
}

/** Text to copy into another app. */
function Snippet({ label, hint, text }: { label: string; hint?: string; text: string }) {
  const t = useT();
  const copy = () =>
    navigator.clipboard
      .writeText(text)
      .then(() => toast(t("common.copied")))
      .catch(() => toast(t("settings.agents.snippet.copyFailed")));
  return (
    <div>
      <div className="label">{label}</div>
      {hint && <p className="mb-1 text-xs text-slate-500">{hint}</p>}
      <div className="flex items-start gap-2">
        <pre className="selectable min-w-0 flex-1 overflow-x-auto rounded-md bg-slate-100 px-3 py-2 font-mono text-xs dark:bg-slate-900">
          {text}
        </pre>
        <button
          className="btn px-2 py-1 text-xs"
          onClick={copy}
          aria-label={t("settings.agents.snippet.copy", { label })}
        >
          <Copy className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}

function DriverSettings({ device, disabled }: { device: string | null; disabled: boolean }) {
  const t = useT();
  const [state, setState] = useState<"open" | "saved" | "unchanged" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const configure = () => {
    if (!device) return;
    setState("open");
    setError(null);
    api
      .scannerDriverSettings(device)
      .then((saved) => setState(saved ? "saved" : "unchanged"))
      .catch((e) => {
        setState(null);
        setError(errorText(e));
      });
  };
  return (
    <div className="mt-2 space-y-1">
      <div className="flex items-center gap-3">
        <button className="btn" onClick={configure} disabled={disabled || !device || state === "open"}>
          <SlidersHorizontal className="h-4 w-4" /> {t("settings.scanner.driver.button")}
        </button>
        <span className="text-xs text-slate-500">
          {state === "open"
            ? t("settings.scanner.driver.open")
            : state === "saved"
              ? t("settings.scanner.driver.saved")
              : state === "unchanged"
                ? t("settings.scanner.driver.unchanged")
                : device
                  ? t("settings.scanner.driver.hint")
                  : t("settings.scanner.driver.chooseFirst")}
        </span>
      </div>
      <ErrorNote>{error}</ErrorNote>
    </div>
  );
}

function About() {
  const t = useT();
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    api.appInfo().then(setInfo);
  }, []);
  return (
    <Section
      title={t("settings.about.title")}
      text={rich(t("settings.about.text"), {
        link: (s) => (
          <button
            className="text-accent underline"
            onClick={() => openUrl("https://github.com/Ximilar-com/cardloader/releases")}
          >
            {s}
          </button>
        ),
      })}
    >
      {info && (
        <dl className="grid grid-cols-[120px_1fr] items-center gap-x-4 gap-y-2 text-sm">
          <dt className="text-slate-500">{t("settings.about.version")}</dt>
          <dd>{info.version}</dd>
          <dt className="text-slate-500">{t("settings.about.dataFolder")}</dt>
          <dd className="flex min-w-0 items-center gap-2">
            <span className="selectable truncate font-mono text-xs" title={info.dataDir}>
              {info.dataDir}
            </span>
            <button className="btn px-2 py-0.5 text-xs" onClick={() => openPath(info.dataDir)}>
              <FolderOpen className="h-3.5 w-3.5" /> {t("common.open")}
            </button>
          </dd>
          <dt className="text-slate-500">{t("settings.about.spaceUsed")}</dt>
          <dd>{t("settings.about.space", { size: bytes(info.dataBytes) })}</dd>
        </dl>
      )}
    </Section>
  );
}

type Tab = "ximilar" | "models" | "sessions" | "ebay" | "scanner" | "agents" | "language" | "about";

function tabs(): { key: Tab; label: string; icon: LucideIcon }[] {
  return [
    { key: "ximilar", label: t("settings.tabs.ximilar"), icon: KeyRound },
    { key: "models", label: t("settings.tabs.models"), icon: Cpu },
    { key: "sessions", label: t("settings.tabs.sessions"), icon: SlidersHorizontal },
    { key: "ebay", label: t("settings.tabs.ebay"), icon: Store },
    { key: "scanner", label: t("settings.tabs.scanner"), icon: ScanLine },
    { key: "agents", label: t("settings.tabs.agents"), icon: Bot },
    { key: "language", label: t("common.language"), icon: Languages },
    { key: "about", label: t("settings.tabs.about"), icon: Info },
  ];
}

function strip(s: Settings | SettingsView): Settings {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const { tokenSet, tokenStorage, ebayCertSet, ebayConnected, imgbbKeySet, ...rest } = s as SettingsView;
  return rest;
}

function EbayTab({
  view,
  draft,
  setEbay,
  applyView,
  setError,
}: {
  view: SettingsView;
  draft: Settings;
  setEbay: (patch: Partial<EbaySettings>) => void;
  applyView: (v: SettingsView) => void;
  setError: (e: string | null) => void;
}) {
  const t = useT();
  const [cert, setCert] = useState("");
  const [imgbbKey, setImgbbKey] = useState("");
  const [connecting, setConnecting] = useState(false);
  /** The pasted consent address; null while the paste field is hidden. */
  const [pasted, setPasted] = useState<string | null>(null);
  const [setup, setSetup] = useState<{ for: string; setup: EbaySetup | null; error: string | null } | null>(null);
  const [reloads, setReloads] = useState(0);
  const [place, setPlace] = useState<{ postalCode: string; country: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const saved = view.ebay;
  const ebay = draft.ebay;
  const keysChanged = ebay.appId !== saved.appId || ebay.ruName !== saved.ruName || ebay.sandbox !== saved.sandbox;
  const keysReady = !!(saved.appId && saved.ruName && view.ebayCertSet) && !keysChanged;
  // The setup belongs to one account (and reload): an older answer isn't shown.
  const account = `${saved.username ?? ""}/${saved.sandbox}/${reloads}`;

  useEffect(() => {
    if (!view.ebayConnected) return;
    let alive = true;
    api
      .ebaySetup()
      .then((s) => alive && setSetup({ for: account, setup: s, error: null }))
      .catch((e) => alive && setSetup({ for: account, setup: null, error: errorText(e) }));
    return () => {
      alive = false;
    };
  }, [view.ebayConnected, account]);

  const current = view.ebayConnected && setup?.for === account ? setup : null;

  const saveCert = async (value: string) => {
    setError(null);
    try {
      applyView(await api.setEbayCert(value));
      setCert("");
    } catch (e) {
      setError(errorText(e));
    }
  };

  const saveImgbbKey = async (value: string) => {
    setError(null);
    try {
      applyView(await api.setImgbbKey(value));
      setImgbbKey("");
    } catch (e) {
      setError(errorText(e));
    }
  };

  const connect = async (text?: string) => {
    setError(null);
    setConnecting(true);
    try {
      applyView(text === undefined ? await api.ebayConnect() : await api.ebayConnectWith(text));
      setPasted(null);
    } catch (e) {
      setError(errorText(e));
      setPasted((p) => p ?? "");
    } finally {
      setConnecting(false);
    }
  };

  const act = async (run: () => Promise<EbaySetup>) => {
    setError(null);
    setBusy(true);
    try {
      setSetup({ for: account, setup: await run(), error: null });
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const addLocation = (p: { postalCode: string; country: string }) =>
    act(async () => {
      const s = await api.ebayCreateLocation(p.postalCode, p.country);
      const v = await api.getSettings();
      applyView(v);
      setEbay({ locationKey: v.ebay.locationKey });
      setPlace(null);
      return s;
    });

  const named = (list: { id: string; name: string }[] | undefined, value: string) => [
    { value: "", label: t("settings.ebay.listings.choose"), disabled: true },
    ...(list ?? []).map((p) => ({ value: p.id, label: p.name })),
    ...(value && !list?.some((p) => p.id === value) ? [{ value, label: value }] : []),
  ];

  return (
    <>
      <Section
        title={t("settings.ebay.keys.title")}
        done={keysReady}
        text={rich(t("settings.ebay.keys.text"), {
          link: (s) => (
            <button className="text-accent underline" onClick={() => openUrl("https://developer.ebay.com/my/keys")}>
              {s}
            </button>
          ),
          b: (s) => <b>{s}</b>,
        })}
      >
        <div className="grid grid-cols-2 gap-4">
          <Field
            label={t("settings.ebay.keys.environment")}
            value={ebay.sandbox ? "sandbox" : "production"}
            options={[
              {
                value: "production",
                label: t("settings.ebay.keys.production"),
                hint: t("settings.ebay.keys.productionHint"),
              },
              { value: "sandbox", label: t("settings.ebay.keys.sandbox"), hint: t("settings.ebay.keys.sandboxHint") },
            ]}
            onChange={(v) => setEbay({ sandbox: v === "sandbox" })}
          />
          <div>
            <label className="label">{t("settings.ebay.keys.appId")}</label>
            <input
              className="input font-mono"
              value={ebay.appId}
              onChange={(e) => setEbay({ appId: e.target.value })}
              spellCheck={false}
            />
          </div>
          <div className="col-span-2">
            <label className="label">{t("settings.ebay.keys.ruName")}</label>
            <input
              className="input font-mono"
              value={ebay.ruName}
              onChange={(e) => setEbay({ ruName: e.target.value })}
              spellCheck={false}
            />
          </div>
          <div className="col-span-2">
            <label className="label">{t("settings.ebay.keys.certId")}</label>
            <div className="flex gap-2">
              <input
                type="password"
                className="input font-mono"
                placeholder={
                  view.ebayCertSet ? t("settings.savedPlaceholder") : t("settings.ebay.keys.certPlaceholder")
                }
                value={cert}
                onChange={(e) => setCert(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && cert && saveCert(cert)}
              />
              <button className="btn btn-primary" disabled={!cert} onClick={() => saveCert(cert)}>
                <KeyRound className="h-4 w-4" /> {t("common.save")}
              </button>
              {view.ebayCertSet && (
                <button className="btn" onClick={() => saveCert("")}>
                  {t("common.remove")}
                </button>
              )}
            </div>
            <p className="mt-1 text-xs text-slate-500">{t("settings.ebay.keys.certNote")}</p>
          </div>
        </div>
      </Section>

      <Section
        title={t("settings.ebay.account.title")}
        done={view.ebayConnected}
        text={t("settings.ebay.account.text")}
      >
        {view.ebayConnected ? (
          <div className="flex items-center gap-3">
            <Status
              ok
              text={saved.username ? t("settings.connectedAs", { account: saved.username }) : t("settings.connected")}
            />
            <div className="flex-1" />
            <button
              className="btn"
              onClick={async () => {
                setError(null);
                try {
                  applyView(await api.ebayDisconnect());
                } catch (e) {
                  setError(errorText(e));
                }
              }}
            >
              <LogOut className="h-4 w-4" /> {t("settings.ebay.account.disconnect")}
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="flex items-center gap-3">
              <button className="btn btn-primary" disabled={!keysReady || connecting} onClick={() => connect()}>
                <LogIn className="h-4 w-4" />{" "}
                {connecting ? t("settings.ebay.account.waiting") : t("settings.ebay.account.connect")}
              </button>
              <span className="text-xs text-slate-500">
                {keysChanged
                  ? t("settings.ebay.account.saveKeysFirst")
                  : !keysReady
                    ? t("settings.ebay.account.enterKeysFirst")
                    : connecting
                      ? t("settings.ebay.account.finishSigningIn")
                      : ""}
              </span>
              <div className="flex-1" />
              {pasted === null && keysReady && (
                <button className="text-xs text-accent hover:underline" onClick={() => setPasted("")}>
                  {t("settings.ebay.account.pasteInstead")}
                </button>
              )}
            </div>
            {pasted !== null && (
              <div>
                <label className="label">{t("settings.ebay.account.pasted")}</label>
                <div className="flex gap-2">
                  <input
                    className="input font-mono text-xs"
                    value={pasted}
                    onChange={(e) => setPasted(e.target.value)}
                    placeholder="https://…&code=…"
                    spellCheck={false}
                  />
                  <button
                    className="btn"
                    disabled={!pasted.trim() || connecting || !keysReady}
                    onClick={() => connect(pasted)}
                  >
                    {t("settings.ebay.account.connectButton")}
                  </button>
                </div>
                <p className="mt-1 text-xs text-slate-500">{t("settings.ebay.account.pasteHelp")}</p>
              </div>
            )}
          </div>
        )}
      </Section>

      {view.ebayConnected && (
        <Section
          title={t("settings.ebay.listings.title")}
          done={ebayReady(saved)}
          text={t("settings.ebay.listings.text")}
        >
          {!current ? (
            <p className="text-sm text-slate-500">{t("settings.ebay.listings.loading")}</p>
          ) : current.error ? (
            <ErrorNote>{current.error}</ErrorNote>
          ) : (
            current.setup && (
              <>
                {!current.setup.optedIn && (
                  <div className="flex items-center gap-3 rounded-md bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:bg-amber-950/40 dark:text-amber-300">
                    <span className="flex-1">{t("settings.ebay.listings.policiesOff")}</span>
                    <button className="btn shrink-0" disabled={busy} onClick={() => act(api.ebayOptIn)}>
                      {t("settings.ebay.listings.turnOn")}
                    </button>
                  </div>
                )}
                <div className="grid grid-cols-2 gap-4">
                  <Field
                    label={t("settings.ebay.listings.shipping")}
                    value={ebay.fulfillmentPolicyId}
                    options={named(current.setup.fulfillmentPolicies, ebay.fulfillmentPolicyId)}
                    onChange={(v) => setEbay({ fulfillmentPolicyId: v })}
                  />
                  <Field
                    label={t("settings.ebay.listings.payment")}
                    value={ebay.paymentPolicyId}
                    options={named(current.setup.paymentPolicies, ebay.paymentPolicyId)}
                    onChange={(v) => setEbay({ paymentPolicyId: v })}
                  />
                  <Field
                    label={t("settings.ebay.listings.return")}
                    value={ebay.returnPolicyId}
                    options={named(current.setup.returnPolicies, ebay.returnPolicyId)}
                    onChange={(v) => setEbay({ returnPolicyId: v })}
                  />
                  <div>
                    <div className="label">{t("settings.ebay.listings.shipsFrom")}</div>
                    <div className="flex gap-2">
                      <Select
                        value={ebay.locationKey}
                        options={named(current.setup.locations, ebay.locationKey).map((o) => ({
                          ...o,
                          icon: o.value ? MapPin : undefined,
                        }))}
                        onChange={(v) => setEbay({ locationKey: v })}
                        aria-label={t("settings.ebay.listings.shipsFrom")}
                      />
                      <button
                        className="btn shrink-0 px-2"
                        title={t("settings.ebay.listings.addLocationTitle")}
                        onClick={() => setPlace(place ? null : { postalCode: "", country: "US" })}
                      >
                        <Plus className="h-4 w-4" />
                      </button>
                    </div>
                  </div>
                </div>
                {place && (
                  <div className="flex items-end gap-2">
                    <div className="flex-1">
                      <label className="label">{t("settings.ebay.listings.postalCode")}</label>
                      <input
                        className="input"
                        value={place.postalCode}
                        onChange={(e) => setPlace({ ...place, postalCode: e.target.value })}
                      />
                    </div>
                    <div className="w-28">
                      <label className="label">{t("settings.ebay.listings.country")}</label>
                      <input
                        className="input uppercase"
                        maxLength={2}
                        value={place.country}
                        onChange={(e) => setPlace({ ...place, country: e.target.value })}
                      />
                    </div>
                    <button
                      className="btn btn-primary"
                      disabled={busy || !place.postalCode.trim() || place.country.trim().length !== 2}
                      onClick={() => addLocation(place)}
                    >
                      {t("settings.ebay.listings.addLocation")}
                    </button>
                  </div>
                )}
                <div className="flex items-center gap-3">
                  <button className="btn" onClick={() => setReloads((n) => n + 1)}>
                    <RefreshCw className="h-4 w-4" /> {t("settings.ebay.listings.reload")}
                  </button>
                  <span className="text-xs text-slate-500">{t("settings.ebay.listings.reloadHint")}</span>
                </div>
              </>
            )
          )}
        </Section>
      )}

      <Section
        title={t("settings.ebay.hosting.title")}
        done={view.imgbbKeySet}
        text={rich(t("settings.ebay.hosting.text"), {
          link: (s) => (
            <button className="text-accent underline" onClick={() => openUrl("https://api.imgbb.com/")}>
              {s}
            </button>
          ),
        })}
      >
        <div>
          <label className="label">{t("settings.ebay.hosting.imgbbKey")}</label>
          <div className="flex gap-2">
            <input
              type="password"
              className="input font-mono"
              placeholder={
                view.imgbbKeySet ? t("settings.savedPlaceholder") : t("settings.ebay.hosting.keyPlaceholder")
              }
              value={imgbbKey}
              onChange={(e) => setImgbbKey(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && imgbbKey && saveImgbbKey(imgbbKey)}
            />
            <button className="btn btn-primary" disabled={!imgbbKey} onClick={() => saveImgbbKey(imgbbKey)}>
              <KeyRound className="h-4 w-4" /> {t("common.save")}
            </button>
            {view.imgbbKeySet && (
              <button className="btn" onClick={() => saveImgbbKey("")}>
                {t("common.remove")}
              </button>
            )}
          </div>
        </div>
      </Section>
    </>
  );
}

function Section({
  title,
  text,
  done,
  children,
}: {
  title: string;
  text?: ReactNode;
  /** Shows a green check next to the title. */
  done?: boolean;
  children: ReactNode;
}) {
  const t = useT();
  return (
    <section className="card space-y-4 p-5">
      <div>
        <h2 className="flex items-center gap-1.5 text-base font-semibold">
          {title}
          {done && <CheckCircle2 className="h-4 w-4 text-emerald-600" aria-label={t("settings.tokenSaved")} />}
        </h2>
        {text && <p className="mt-0.5 text-sm text-slate-500">{text}</p>}
      </div>
      {children}
    </section>
  );
}

function Field<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: SelectOption<T>[];
  onChange: (v: T) => void;
}) {
  return (
    <div>
      <div className="label">{label}</div>
      <Select value={value} options={options} onChange={onChange} aria-label={label} />
    </div>
  );
}

function rotationOptions(): SelectOption<Settings["scan"]["rotation"]>[] {
  return [
    { value: "none", label: t("common.none") },
    { value: "upside_down", label: t("settings.scanner.upsideDown"), hint: t("settings.scanner.upsideDownHint") },
  ];
}

function sideOrderOptions(): SelectOption<"front" | "back">[] {
  return [
    { value: "front", label: t("settings.scanner.frontFirst") },
    { value: "back", label: t("settings.scanner.backFirst") },
  ];
}

function sourceOptions(): SelectOption<Settings["defaultSources"][number]>[] {
  const labels = sourceLabels();
  return (["scanner", "folder", "watch_folder", "csv"] as const).map((value) => ({ value, label: labels[value] }));
}

// At most 3, as `settings::MAX_CONCURRENCY`.
function concurrencyOptions(): SelectOption<string>[] {
  return [
    { value: "1", label: "1" },
    { value: "2", label: "2" },
    { value: "3", label: "3", hint: t("settings.sessions.fastest") },
  ];
}

function colorOptions(): SelectOption<Settings["scan"]["color"]>[] {
  return [
    { value: "color", label: t("settings.scanner.colorColor") },
    { value: "gray", label: t("settings.scanner.colorGray") },
    { value: "bw", label: t("settings.scanner.colorBw") },
  ];
}

function Status({ ok, text }: { ok: boolean; text: string }) {
  return (
    <div className={clsx("flex items-center gap-1.5", ok ? "text-emerald-600" : "text-red-600")}>
      {ok ? <CheckCircle2 className="h-4 w-4" /> : <XCircle className="h-4 w-4" />}
      <span className="selectable">{text}</span>
    </div>
  );
}

function EngineOption({
  active,
  onClick,
  icon,
  title,
  badge,
  text,
}: {
  active: boolean;
  onClick: () => void;
  icon: ReactNode;
  title: string;
  badge?: string;
  text: string;
}) {
  return (
    <button
      onClick={onClick}
      className={clsx(
        "flex flex-col items-start gap-1 rounded-lg border p-3 text-left transition-colors",
        active
          ? "border-accent bg-accent/5 ring-2 ring-accent/20"
          : "border-slate-200 hover:border-slate-300 dark:border-slate-700",
      )}
    >
      <span className="flex items-center gap-2 text-accent">
        {icon}
        <span className="text-sm font-medium text-slate-900 dark:text-slate-100">{title}</span>
        {badge && <span className="text-[11px] text-slate-400">{badge}</span>}
      </span>
      <span className="text-xs text-slate-500">{text}</span>
    </button>
  );
}
