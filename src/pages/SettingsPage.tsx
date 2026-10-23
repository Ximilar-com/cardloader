import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import {
  CheckCircle2,
  Cloud,
  Cpu,
  FolderOpen,
  KeyRound,
  LogIn,
  LogOut,
  MapPin,
  Plus,
  RefreshCw,
  ScanLine,
  SlidersHorizontal,
  Store,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useLocation } from "react-router-dom";
import { ebayReady } from "../components/Ebay";
import { ModelPicker, useModels } from "../components/LocalModels";
import { ErrorNote } from "../components/Modal";
import { Check, DPI_OPTIONS, MULTIFEED_OPTIONS, PAIRING_OPTIONS } from "../components/NewSessionDialog";
import Select, { optionsOf, type SelectOption } from "../components/Select";
import {
  api,
  errorText,
  type ConnectionCheck,
  type DeviceInfo,
  type EbaySettings,
  type EbaySetup,
  type ScannerSupport,
  type Settings,
  type SettingsView,
} from "../lib/api";
import { GRADING_LABELS, ID_MODE_LABELS } from "../lib/format";

export default function SettingsPage() {
  const [view, setView] = useState<SettingsView | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [token, setToken] = useState("");
  const [check, setCheck] = useState<ConnectionCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [support, setSupport] = useState<ScannerSupport | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[] | null>(null);
  const [findingScanners, setFindingScanners] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const models = useModels();
  // Another page can open Settings at a tab: navigate("/settings", { state: { section } }).
  const section = (useLocation().state as { section?: Tab } | null)?.section;
  const [tab, setTab] = useState<Tab>(section ?? "ximilar");
  const tokenInput = useRef<HTMLInputElement>(null);
  const loaded = !!view;

  useEffect(() => {
    if (loaded && section === "ximilar") tokenInput.current?.focus();
  }, [loaded, section]);

  useEffect(() => {
    api.getSettings().then((v) => {
      setView(v);
      setDraft(v);
    });
    api.scannerSupport().then(setSupport);
  }, []);

  if (!view || !draft) return null;
  const dirty = JSON.stringify(strip(view)) !== JSON.stringify(strip(draft));
  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setSaved(false);
    setDraft({ ...draft, [key]: value });
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
  const setScan = <K extends keyof Settings["scan"]>(key: K, value: Settings["scan"][K]) =>
    set("scan", { ...draft.scan, [key]: value });

  const save = async () => {
    setError(null);
    try {
      const v = await api.saveSettings(draft);
      setView(v);
      setDraft(v);
      setSaved(true);
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
        <h1 className="text-lg font-semibold">Settings</h1>
        <div className="flex-1" />
        {saved && !dirty && <span className="text-sm text-emerald-600">Saved</span>}
        <button className="btn" disabled={!dirty} onClick={() => setDraft(view)}>
          Discard
        </button>
        <button className="btn btn-primary" disabled={!dirty} onClick={save}>
          Save
        </button>
      </header>
      <div className="flex min-h-0 flex-1">
        <nav
          className="w-52 shrink-0 space-y-0.5 border-r border-slate-200 p-3 dark:border-slate-800"
          aria-label="Settings"
        >
          {TABS.map(({ key, label, icon: Icon }) => (
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
                <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" aria-label="token saved" />
              )}
              {key === "ebay" && view.ebayConnected && ebayReady(view.ebay) && (
                <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" aria-label="eBay connected" />
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
                  title="Ximilar API"
                  done={view.tokenSet}
                  text={
                    <>
                      Cardloader calls the Ximilar Collectibles Recognition and Card Grading APIs with your token. Find
                      it in the{" "}
                      <button className="text-accent underline" onClick={() => openUrl("https://app.ximilar.com")}>
                        Ximilar App
                      </button>{" "}
                      under your profile.
                    </>
                  }
                >
                  <div>
                    <label className="label">API token</label>
                    <div className="flex gap-2">
                      <input
                        ref={tokenInput}
                        type="password"
                        className="input font-mono"
                        placeholder={view.tokenSet ? "•••••••••••••••• (saved)" : "Paste your API token"}
                        value={token}
                        onChange={(e) => setToken(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && token && saveToken(token)}
                      />
                      <button className="btn btn-primary" disabled={!token} onClick={() => saveToken(token)}>
                        <KeyRound className="h-4 w-4" /> Save token
                      </button>
                      {view.tokenSet && (
                        <button className="btn" onClick={() => saveToken("")}>
                          Remove
                        </button>
                      )}
                    </div>
                    <p className="mt-1 text-xs text-slate-500">
                      {view.tokenSet
                        ? view.tokenStorage === "keychain"
                          ? "Stored in the system keychain."
                          : view.tokenStorage === "dev"
                            ? "Development build — stored in the settings file, not the keychain."
                            : "No system keychain available — stored in the settings file."
                        : "No token saved."}
                    </p>
                  </div>
                  <div className="flex items-center gap-3">
                    <button className="btn" disabled={!view.tokenSet || checking} onClick={testConnection}>
                      <RefreshCw className={clsx("h-4 w-4", checking && "animate-spin")} /> Test connection
                    </button>
                    {check && (
                      <div className="space-y-0.5 text-sm">
                        <Status
                          ok={check.ok}
                          text={
                            check.ok
                              ? `Connected${check.account ? ` as ${check.account}` : ""}`
                              : (check.error ?? "Failed")
                          }
                        />
                        <Status
                          ok={check.cardsCloud}
                          text={
                            check.cardsCloud
                              ? "Cards Cloud available"
                              : `Cards Cloud unavailable${check.cardsCloudError ? ` (${check.cardsCloudError})` : ""}`
                          }
                        />
                      </div>
                    )}
                  </div>
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <label className="label">Cards Cloud workspace (optional)</label>
                      <input
                        className="input font-mono"
                        placeholder="default workspace"
                        value={draft.workspace ?? ""}
                        onChange={(e) => set("workspace", e.target.value || null)}
                      />
                    </div>
                    <div>
                      <label className="label">API base URL</label>
                      <input
                        className="input font-mono"
                        value={draft.apiBaseUrl}
                        onChange={(e) => set("apiBaseUrl", e.target.value)}
                      />
                    </div>
                  </div>
                </Section>
              </>
            )}
            {tab === "models" && (
              <Section
                title="Local models"
                text={
                  <>
                    Vision models that identify cards on this computer, free and offline. Larger models read the small
                    print more reliably but are slower and need more memory; <b>⋯ → Compare with local model…</b> on a
                    Ximilar session shows how each does on your cards. The chosen model is the one new sessions use.
                  </>
                }
              >
                {models && (
                  <ModelPicker models={models} value={draft.localModel} onChange={(id) => set("localModel", id)} />
                )}
              </Section>
            )}
            {tab === "sessions" && (
              <>
                <Section
                  title="Identification"
                  text="What identifies the cards in new sessions. Each session keeps its choice."
                >
                  <div className="grid grid-cols-2 gap-3">
                    <EngineOption
                      active={draft.engine === "ximilar"}
                      onClick={() => set("engine", "ximilar")}
                      icon={<Cloud className="h-5 w-5" />}
                      title="Ximilar API"
                      badge="Default"
                      text="The exact card and print from Ximilar's database, with prices, other candidates and grading. Uses API credits; needs internet."
                    />
                    <EngineOption
                      active={draft.engine === "local"}
                      onClick={() => set("engine", "local")}
                      icon={<Cpu className="h-5 w-5" />}
                      title="Local model"
                      text="A small vision model on this computer: free and offline. Reads the name, number and year printed on the card; no prices, candidates or grading."
                    />
                  </div>
                  {draft.engine === "local" && (
                    <p className="text-sm text-slate-500">
                      New sessions use {models?.find((m) => m.id === draft.localModel)?.name ?? draft.localModel}.{" "}
                      <button className="text-accent hover:underline" onClick={() => setTab("models")}>
                        Change it in Local models
                      </button>
                    </p>
                  )}
                </Section>
                <Section title="New sessions" text="Defaults for the New session dialog.">
                  <div className="grid grid-cols-2 gap-4">
                    <div className="col-span-2">
                      <label className="label">Default folder</label>
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
                          <FolderOpen className="h-4 w-4" /> Browse
                        </button>
                      </div>
                    </div>
                    <Field
                      label="Identify as"
                      value={draft.defaultIdMode}
                      options={optionsOf(ID_MODE_LABELS)}
                      onChange={(v) => set("defaultIdMode", v)}
                    />
                    <Field
                      label="Grading"
                      value={draft.defaultGrading}
                      options={optionsOf(GRADING_LABELS)}
                      onChange={(v) => set("defaultGrading", v)}
                    />
                    <Field
                      label="Images per card"
                      value={draft.defaultPairing}
                      options={PAIRING_OPTIONS}
                      onChange={(v) => set("defaultPairing", v)}
                    />
                    <div>
                      <label className="label">Parallel requests</label>
                      <input
                        type="number"
                        min={1}
                        max={16}
                        className="input"
                        value={draft.concurrency}
                        onChange={(e) => set("concurrency", Number(e.target.value) || 1)}
                      />
                    </div>
                    <Check label="Price statistics" checked={draft.priceStats} onChange={(v) => set("priceStats", v)} />
                    <Check
                      label="Sync new sessions to Cards Cloud"
                      checked={draft.syncByDefault}
                      onChange={(v) => set("syncByDefault", v)}
                    />
                  </div>
                </Section>
              </>
            )}
            {tab === "ebay" && (
              <EbayTab view={view} draft={draft} setEbay={setEbay} applyView={applyView} setError={setError} />
            )}
            {tab === "scanner" && (
              <Section
                title="Scanner"
                text={
                  support?.backend === "twain"
                    ? "Windows: scanners are driven through TWAIN (install RICOH PaperStream IP for the fi-8170)."
                    : "Scanners are driven through SANE (scanimage). Connect the scanner over USB."
                }
              >
                {support && !support.available && <ErrorNote>{support.hint}</ErrorNote>}
                <div className="grid grid-cols-2 gap-4">
                  <div className="col-span-2">
                    <label className="label">Default scanner</label>
                    <div className="flex gap-2">
                      <Select
                        value={draft.defaultScanner ?? ""}
                        onChange={(v) => set("defaultScanner", v || null)}
                        aria-label="Default scanner"
                        options={[
                          { value: "", label: "None" },
                          ...(draft.defaultScanner && !devices?.some((d) => d.id === draft.defaultScanner)
                            ? [{ value: draft.defaultScanner, label: draft.defaultScanner, icon: ScanLine }]
                            : []),
                          ...(devices ?? []).map((d) => ({ value: d.id, label: d.name || d.id, icon: ScanLine })),
                        ]}
                      />
                      <button className="btn" onClick={findScanners} disabled={findingScanners || !support?.available}>
                        <RefreshCw className={clsx("h-4 w-4", findingScanners && "animate-spin")} /> Find scanners
                      </button>
                    </div>
                  </div>
                  <Field
                    label="Resolution"
                    value={String(draft.scan.dpi)}
                    options={DPI_OPTIONS}
                    onChange={(v) => setScan("dpi", Number(v))}
                  />
                  <Field
                    label="Colour"
                    value={draft.scan.color}
                    options={COLOR_OPTIONS}
                    onChange={(v) => setScan("color", v)}
                  />
                  <div>
                    <label className="label">Page width (mm)</label>
                    <input
                      type="number"
                      className="input"
                      value={draft.scan.pageWidthMm}
                      onChange={(e) => setScan("pageWidthMm", Number(e.target.value))}
                    />
                  </div>
                  <div>
                    <label className="label">Page height (mm)</label>
                    <input
                      type="number"
                      className="input"
                      value={draft.scan.pageHeightMm}
                      onChange={(e) => setScan("pageHeightMm", Number(e.target.value))}
                    />
                  </div>
                  <Field
                    label="Multifeed detection"
                    value={draft.scan.multifeed}
                    options={MULTIFEED_OPTIONS}
                    onChange={(v) => setScan("multifeed", v)}
                  />
                  <div className="flex items-end pb-2">
                    <Check
                      label="Scan both sides (duplex)"
                      checked={draft.scan.duplex}
                      onChange={(v) => setScan("duplex", v)}
                    />
                  </div>
                </div>
                <p className="text-xs text-slate-500">
                  Standard cards are 63 × 88 mm; a slightly larger frame (66 × 92 mm) keeps the card edges in the scan.
                </p>
              </Section>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

type Tab = "ximilar" | "models" | "sessions" | "ebay" | "scanner";

const TABS: { key: Tab; label: string; icon: LucideIcon }[] = [
  { key: "ximilar", label: "Ximilar API", icon: KeyRound },
  { key: "models", label: "Local models", icon: Cpu },
  { key: "sessions", label: "New sessions", icon: SlidersHorizontal },
  { key: "ebay", label: "Selling", icon: Store },
  { key: "scanner", label: "Scanner", icon: ScanLine },
];

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
    { value: "", label: "Choose…", disabled: true },
    ...(list ?? []).map((p) => ({ value: p.id, label: p.name })),
    ...(value && !list?.some((p) => p.id === value) ? [{ value, label: value }] : []),
  ];

  return (
    <>
      <Section
        title="eBay developer keys"
        done={keysReady}
        text={
          <>
            Cardloader lists cards with your own keys from the{" "}
            <button className="text-accent underline" onClick={() => openUrl("https://developer.ebay.com/my/keys")}>
              eBay developer program
            </button>
            . Create a keyset, then under <b>User Tokens</b> add an eBay Redirect URL (RuName) and keep eBay’s own “auth
            accepted” page. Production keys work once eBay activates them, which needs the Marketplace Account Deletion
            exemption (or notifications).
          </>
        }
      >
        <div className="grid grid-cols-2 gap-4">
          <Field
            label="Environment"
            value={ebay.sandbox ? "sandbox" : "production"}
            options={[
              { value: "production", label: "Production", hint: "real listings" },
              { value: "sandbox", label: "Sandbox", hint: "test listings" },
            ]}
            onChange={(v) => setEbay({ sandbox: v === "sandbox" })}
          />
          <div>
            <label className="label">App ID (Client ID)</label>
            <input
              className="input font-mono"
              value={ebay.appId}
              onChange={(e) => setEbay({ appId: e.target.value })}
              spellCheck={false}
            />
          </div>
          <div className="col-span-2">
            <label className="label">RuName (eBay Redirect URL name)</label>
            <input
              className="input font-mono"
              value={ebay.ruName}
              onChange={(e) => setEbay({ ruName: e.target.value })}
              spellCheck={false}
            />
          </div>
          <div className="col-span-2">
            <label className="label">Cert ID (Client Secret)</label>
            <div className="flex gap-2">
              <input
                type="password"
                className="input font-mono"
                placeholder={view.ebayCertSet ? "•••••••••••••••• (saved)" : "Paste the Cert ID"}
                value={cert}
                onChange={(e) => setCert(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && cert && saveCert(cert)}
              />
              <button className="btn btn-primary" disabled={!cert} onClick={() => saveCert(cert)}>
                <KeyRound className="h-4 w-4" /> Save
              </button>
              {view.ebayCertSet && (
                <button className="btn" onClick={() => saveCert("")}>
                  Remove
                </button>
              )}
            </div>
            <p className="mt-1 text-xs text-slate-500">Kept with the API token; changing the keys disconnects eBay.</p>
          </div>
        </div>
      </Section>

      <Section
        title="eBay account"
        done={view.ebayConnected}
        text="Sign in to the eBay seller account the cards are listed on. eBay asks you to agree once; the login lasts about 18 months."
      >
        {view.ebayConnected ? (
          <div className="flex items-center gap-3">
            <Status ok text={`Connected${saved.username ? ` as ${saved.username}` : ""}`} />
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
              <LogOut className="h-4 w-4" /> Disconnect
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="flex items-center gap-3">
              <button className="btn btn-primary" disabled={!keysReady || connecting} onClick={() => connect()}>
                <LogIn className="h-4 w-4" /> {connecting ? "Waiting for eBay…" : "Connect eBay account"}
              </button>
              <span className="text-xs text-slate-500">
                {keysChanged
                  ? "Save the keys first."
                  : !keysReady
                    ? "Enter the keys above first."
                    : connecting
                      ? "Finish signing in in the eBay window."
                      : ""}
              </span>
              <div className="flex-1" />
              {pasted === null && keysReady && (
                <button className="text-xs text-accent hover:underline" onClick={() => setPasted("")}>
                  Paste the address instead
                </button>
              )}
            </div>
            {pasted !== null && (
              <div>
                <label className="label">Address eBay showed after you agreed</label>
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
                    Connect
                  </button>
                </div>
                <p className="mt-1 text-xs text-slate-500">
                  If the window can’t finish: open eBay’s sign-in in your browser, agree, and paste the address of the
                  page it ends on.
                </p>
              </div>
            )}
          </div>
        )}
      </Section>

      {view.ebayConnected && (
        <Section
          title="Listings"
          done={ebayReady(saved)}
          text="What new listings use. Cards are listed on eBay US (EBAY_US), priced in US dollars, one of each, at a fixed price until sold."
        >
          {!current ? (
            <p className="text-sm text-slate-500">Loading your eBay account…</p>
          ) : current.error ? (
            <ErrorNote>{current.error}</ErrorNote>
          ) : (
            current.setup && (
              <>
                {!current.setup.optedIn && (
                  <div className="flex items-center gap-3 rounded-md bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:bg-amber-950/40 dark:text-amber-300">
                    <span className="flex-1">
                      Business policies are off for this account; listings need them. Turn them on, then create
                      shipping, payment and return policies in Seller Hub.
                    </span>
                    <button className="btn shrink-0" disabled={busy} onClick={() => act(api.ebayOptIn)}>
                      Turn on business policies
                    </button>
                  </div>
                )}
                <div className="grid grid-cols-2 gap-4">
                  <Field
                    label="Shipping policy"
                    value={ebay.fulfillmentPolicyId}
                    options={named(current.setup.fulfillmentPolicies, ebay.fulfillmentPolicyId)}
                    onChange={(v) => setEbay({ fulfillmentPolicyId: v })}
                  />
                  <Field
                    label="Payment policy"
                    value={ebay.paymentPolicyId}
                    options={named(current.setup.paymentPolicies, ebay.paymentPolicyId)}
                    onChange={(v) => setEbay({ paymentPolicyId: v })}
                  />
                  <Field
                    label="Return policy"
                    value={ebay.returnPolicyId}
                    options={named(current.setup.returnPolicies, ebay.returnPolicyId)}
                    onChange={(v) => setEbay({ returnPolicyId: v })}
                  />
                  <div>
                    <div className="label">Ships from</div>
                    <div className="flex gap-2">
                      <Select
                        value={ebay.locationKey}
                        options={named(current.setup.locations, ebay.locationKey).map((o) => ({
                          ...o,
                          icon: o.value ? MapPin : undefined,
                        }))}
                        onChange={(v) => setEbay({ locationKey: v })}
                        aria-label="Ships from"
                      />
                      <button
                        className="btn shrink-0 px-2"
                        title="Add a location"
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
                      <label className="label">Postal code</label>
                      <input
                        className="input"
                        value={place.postalCode}
                        onChange={(e) => setPlace({ ...place, postalCode: e.target.value })}
                      />
                    </div>
                    <div className="w-28">
                      <label className="label">Country</label>
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
                      Add location
                    </button>
                  </div>
                )}
                <div className="flex items-center gap-3">
                  <button className="btn" onClick={() => setReloads((n) => n + 1)}>
                    <RefreshCw className="h-4 w-4" /> Reload policies
                  </button>
                  <span className="text-xs text-slate-500">Changed policies on eBay? Reload them here, then Save.</span>
                </div>
              </>
            )
          )}
        </Section>
      )}

      <Section
        title="Picture hosting"
        done={view.imgbbKeySet}
        text={
          <>
            Whatnot’s CSV import downloads each card’s pictures from a public link, so the Whatnot export uploads them
            to{" "}
            <button className="text-accent underline" onClick={() => openUrl("https://api.imgbb.com/")}>
              ImgBB
            </button>{" "}
            first (free; get an API key there). Uploaded pictures are public to anyone with the link.
          </>
        }
      >
        <div>
          <label className="label">ImgBB API key</label>
          <div className="flex gap-2">
            <input
              type="password"
              className="input font-mono"
              placeholder={view.imgbbKeySet ? "•••••••••••••••• (saved)" : "Paste the API key"}
              value={imgbbKey}
              onChange={(e) => setImgbbKey(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && imgbbKey && saveImgbbKey(imgbbKey)}
            />
            <button className="btn btn-primary" disabled={!imgbbKey} onClick={() => saveImgbbKey(imgbbKey)}>
              <KeyRound className="h-4 w-4" /> Save
            </button>
            {view.imgbbKeySet && (
              <button className="btn" onClick={() => saveImgbbKey("")}>
                Remove
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
  return (
    <section className="card space-y-4 p-5">
      <div>
        <h2 className="flex items-center gap-1.5 text-base font-semibold">
          {title}
          {done && <CheckCircle2 className="h-4 w-4 text-emerald-600" aria-label="token saved" />}
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

const COLOR_OPTIONS: SelectOption<Settings["scan"]["color"]>[] = [
  { value: "color", label: "Colour" },
  { value: "gray", label: "Greyscale" },
  { value: "bw", label: "Black & white" },
];

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
