import { open } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import { FolderOpen } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { api, errorText, type Engine, type GradingMode, type Prices, type Session } from "../lib/api";
import { ebaySiteOptions } from "../lib/ebaySites";
import { cropEngineOf, gradeEngineOf } from "../lib/format";
import { modelOption, useModels } from "./LocalModels";
import Modal, { ErrorNote } from "./Modal";
import {
  Check,
  conditionOptions,
  conditionScale,
  cropFormatOptions,
  engineOptions,
  gradeModeOptions,
  localConditionOptions,
  OpenSettings,
  priceOptions,
} from "./NewSessionDialog";
import Select from "./Select";
import { toast } from "./Toast";

type Mode = Exclude<GradingMode, "off">;

/**
 * Prices, grading or crops for a finished session's identified cards (`itemIds`, else all of them): they go through
 * just that, keeping their identification and review, and the session keeps the task for cards it gets later.
 * Ximilar's prices aren't offered: they come only with its identification.
 */
export default function AddToCardsDialog({
  session,
  itemIds,
  onClose,
}: {
  session: Session;
  itemIds?: number[];
  onClose: () => void;
}) {
  const t = useT();
  const o = session.options;
  const [prices, setPrices] = useState(true);
  const [pricesBy, setPricesBy] = useState<"ebay" | "agent">(o.engine === "agent" ? "agent" : "ebay");
  const [grade, setGrade] = useState(false);
  const [gradeBy, setGradeBy] = useState<Engine>(gradeEngineOf(o));
  const [mode, setMode] = useState<Mode>(o.grading === "off" ? "grade" : o.grading);
  const [scale, setScale] = useState(o.conditionMode);
  const [model, setModel] = useState(o.localModel);
  const [crops, setCrops] = useState(false);
  const [cropBy, setCropBy] = useState<Engine>(cropEngineOf(o));
  const [cropFolder, setCropFolder] = useState(o.cropFolder);
  const [cropFormat, setCropFormat] = useState(o.cropFormat);
  const [cropLevel, setCropLevel] = useState(o.cropLevel);
  const [site, setSite] = useState<string | null>(o.ebaySite || null);
  const [defaultSite, setDefaultSite] = useState("EBAY_US");
  const [ready, setReady] = useState<{ token: boolean; ebay: boolean; agents: boolean } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const models = useModels();

  useEffect(() => {
    api
      .getSettings()
      .then((s) => {
        setReady({ token: s.tokenSet, ebay: s.ebaySearch, agents: s.mcp.enabled && s.mcp.allowRun });
        setDefaultSite(s.ebay.searchMarketplace || "EBAY_US");
        // Where the last session's crops went, unless this one has a folder of its own.
        setCropFolder((folder) => folder || (s.defaultCropFolder ?? ""));
      })
      .catch((e) => setError(errorText(e)));
  }, []);

  const gradesLocally = grade && gradeBy === "local";
  const cropsLocally = crops && cropBy === "local";
  const local = gradesLocally || cropsLocally;
  // Crops go to a folder of the user's: the session's own folder would be deleted with the session.
  const needsCropFolder = crops && !cropFolder.trim();
  const downloaded = models?.filter((m) => m.state === "ready") ?? [];
  const chosenModel = downloaded.find((m) => m.id === model) ?? downloaded[0];
  // What the run would refuse, said before it is started.
  const missing = [
    prices && pricesBy === "ebay" && ready && !ready.ebay && { text: t("newSession.needsEbay"), section: "ebay" },
    prices &&
      pricesBy === "agent" &&
      ready &&
      !ready.agents && { text: t("newSession.needsAgentsPrices"), section: "agents" },
    crops &&
      cropBy === "agent" &&
      ready &&
      !ready.agents && { text: t("newSession.needsAgentsCrop"), section: "agents" },
    local && models && !chosenModel && { text: t("newSession.needsModel"), section: "models" },
    ((grade && gradeBy === "ximilar") || (crops && cropBy === "ximilar")) &&
      ready &&
      !ready.token && { text: t("newSession.needsToken"), section: "ximilar" },
  ].filter((m): m is { text: string; section: string } => !!m);
  const count = itemIds?.length;
  const pickCropFolder = async () => {
    const picked = await open({ directory: true, defaultPath: cropFolder || undefined });
    if (typeof picked === "string") setCropFolder(picked);
  };
  /** One model does whatever runs on this computer. */
  const modelField = (
    <div>
      <label className="label">{t("newSession.model")}</label>
      <Select
        value={chosenModel?.id ?? ""}
        onChange={setModel}
        placeholder={t("newSession.noModel")}
        options={(models ?? []).map((m) =>
          m.state === "ready"
            ? modelOption(m)
            : { ...modelOption(m), disabled: true, hint: t("newSession.notDownloaded") },
        )}
      />
    </div>
  );

  const run = () => {
    setBusy(true);
    setError(null);
    api
      .addToCards(session.id, itemIds ?? null, {
        prices: prices ? pricesBy : null,
        grading: grade ? mode : null,
        gradeEngine: grade ? gradeBy : null,
        conditionMode: grade ? conditionScale(scale, gradesLocally) : null,
        localModel: local ? (chosenModel?.id ?? null) : null,
        ebaySite: prices ? (site ?? defaultSite) : null,
        crops,
        cropEngine: crops ? cropBy : null,
        cropFolder: crops ? cropFolder.trim() : null,
        cropFormat: crops ? cropFormat : null,
        cropLevel: crops ? cropLevel : null,
      })
      .then((n) => {
        toast(n ? t("session.addTo.started", { count: n }) : t("session.addTo.none"));
        onClose();
      })
      .catch((e) => {
        setError(errorText(e));
        setBusy(false);
      });
  };

  return (
    <Modal
      title={t("session.addTo.title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button
            className="btn btn-primary"
            onClick={run}
            disabled={busy || !ready || (!prices && !grade && !crops) || missing.length > 0 || needsCropFolder}
          >
            {t("session.addTo.run")}
          </button>
        </>
      }
    >
      <div className="space-y-5">
        <p className="text-sm text-slate-600 dark:text-slate-300">
          {count == null ? t("session.addTo.introAll") : t("session.addTo.intro", { count })}
        </p>

        <Part on={prices} onToggle={setPrices} label={t("newSession.tasks.prices")}>
          <div>
            <label className="label">{t("newSession.priceWith")}</label>
            <Select
              value={pricesBy}
              onChange={(v: Prices) => (v === "ebay" || v === "agent") && setPricesBy(v)}
              // Ximilar's prices come with its identification: not for cards identified already.
              options={priceOptions(false).map((p) =>
                p.value === "ximilar" ? { ...p, hint: t("session.addTo.ximilarNewOnly") } : p,
              )}
            />
          </div>
          <div>
            <label className="label">{t("detail.market.site")}</label>
            <Select value={site ?? defaultSite} onChange={setSite} options={ebaySiteOptions()} />
          </div>
          <p className="col-span-2 -mt-2 text-xs text-slate-500">{t(`newSession.priceNotes.${pricesBy}`)}</p>
        </Part>

        <Part on={grade} onToggle={setGrade} label={t("newSession.tasks.grade")}>
          <div>
            <label className="label">{t("newSession.gradeWith")}</label>
            <Select value={gradeBy} onChange={setGradeBy} options={engineOptions(false)} />
          </div>
          {gradesLocally ? modelField : <div />}
          <div>
            <label className="label">{t("newSession.gradeWhat")}</label>
            <Select value={mode} onChange={setMode} options={gradeModeOptions()} />
          </div>
          {mode === "condition" || (gradesLocally && mode === "grade") ? (
            <div>
              <label className="label">{t("newSession.conditionScale")}</label>
              <Select
                value={conditionScale(scale, gradesLocally)}
                onChange={setScale}
                options={gradesLocally ? localConditionOptions() : conditionOptions()}
              />
            </div>
          ) : (
            <div />
          )}
        </Part>

        <Part on={crops} onToggle={setCrops} label={t("newSession.tasks.crop")}>
          <div>
            <label className="label">{t("newSession.cropWith")}</label>
            <Select value={cropBy} onChange={setCropBy} options={engineOptions()} />
          </div>
          {cropsLocally && !gradesLocally ? modelField : <div />}
          <p className="col-span-2 -mt-2 text-xs text-slate-500">{t(`newSession.cropNotes.${cropBy}`)}</p>
          <div className="col-span-2">
            <label className="label">{t("newSession.crop.folder")}</label>
            <div className="flex gap-2">
              <input
                className={clsx(
                  "input",
                  needsCropFolder && "border-amber-400 ring-2 ring-amber-400/25 dark:border-amber-500",
                )}
                value={cropFolder}
                onChange={(e) => setCropFolder(e.target.value)}
                placeholder="/path/to/cropped"
              />
              <button className={clsx("btn", needsCropFolder && "btn-primary")} onClick={pickCropFolder}>
                <FolderOpen className="h-4 w-4" /> {t("newSession.browse")}
              </button>
            </div>
            <p
              className={clsx(
                "mt-1 text-xs",
                needsCropFolder ? "text-amber-700 dark:text-amber-400" : "text-slate-500",
              )}
            >
              {needsCropFolder ? t("newSession.crop.choose") : t("newSession.crop.hint")}
            </p>
          </div>
          <div>
            <label className="label">{t("newSession.crop.format")}</label>
            <Select value={cropFormat} onChange={setCropFormat} options={cropFormatOptions(cropBy)} />
          </div>
          {cropBy === "ximilar" && (
            <div className="col-span-2">
              <Check label={t("newSession.crop.level")} checked={cropLevel} onChange={setCropLevel} />
              <p className="mt-1 pl-6 text-xs text-slate-500">{t("newSession.crop.levelHint")}</p>
            </div>
          )}
        </Part>

        {missing.map((m) => (
          <ErrorNote key={m.text}>
            <div className="flex items-center gap-3">
              <span className="flex-1">{m.text}</span>
              <OpenSettings section={m.section} onOpen={onClose} />
            </div>
          </ErrorNote>
        ))}
        <ErrorNote>{error}</ErrorNote>
      </div>
    </Modal>
  );
}

/** One thing to add: ticked, its fields below. */
function Part({
  on,
  onToggle,
  label,
  children,
}: {
  on: boolean;
  onToggle: (on: boolean) => void;
  label: string;
  children: ReactNode;
}) {
  return (
    <div className={clsx("rounded-lg border p-3", on ? "border-accent/40" : "border-slate-200 dark:border-slate-700")}>
      <Check label={label} checked={on} onChange={onToggle} />
      {on && <div className="mt-3 grid grid-cols-2 gap-4">{children}</div>}
    </div>
  );
}
