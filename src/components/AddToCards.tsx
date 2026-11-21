import clsx from "clsx";
import { useEffect, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { api, errorText, type Engine, type GradingMode, type Prices, type Session } from "../lib/api";
import { gradeEngineOf } from "../lib/format";
import { modelOption, useModels } from "./LocalModels";
import Modal, { ErrorNote } from "./Modal";
import {
  Check,
  conditionOptions,
  conditionScale,
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
 * Prices or grading for a finished session's identified cards (`itemIds`, else all of them): they go through just
 * that, keeping their identification and review, and the session keeps the task for cards it gets later.
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
  const [ready, setReady] = useState<{ token: boolean; ebay: boolean; agents: boolean } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const models = useModels();

  useEffect(() => {
    api
      .getSettings()
      .then((s) => setReady({ token: s.tokenSet, ebay: s.ebaySearch, agents: s.mcp.enabled && s.mcp.allowRun }))
      .catch((e) => setError(errorText(e)));
  }, []);

  const local = grade && gradeBy === "local";
  const downloaded = models?.filter((m) => m.state === "ready") ?? [];
  const chosenModel = downloaded.find((m) => m.id === model) ?? downloaded[0];
  // What the run would refuse, said before it is started.
  const missing = [
    prices && pricesBy === "ebay" && ready && !ready.ebay && { text: t("newSession.needsEbay"), section: "ebay" },
    prices &&
      pricesBy === "agent" &&
      ready &&
      !ready.agents && { text: t("newSession.needsAgentsPrices"), section: "agents" },
    local && models && !chosenModel && { text: t("newSession.needsModel"), section: "models" },
    grade && gradeBy === "ximilar" && ready && !ready.token && { text: t("newSession.needsToken"), section: "ximilar" },
  ].filter((m): m is { text: string; section: string } => !!m);
  const count = itemIds?.length;

  const run = () => {
    setBusy(true);
    setError(null);
    api
      .addToCards(session.id, itemIds ?? null, {
        prices: prices ? pricesBy : null,
        grading: grade ? mode : null,
        gradeEngine: grade ? gradeBy : null,
        conditionMode: grade ? conditionScale(scale, local) : null,
        localModel: local ? (chosenModel?.id ?? null) : null,
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
            disabled={busy || !ready || (!prices && !grade) || missing.length > 0}
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
          <div />
          <p className="col-span-2 -mt-2 text-xs text-slate-500">{t(`newSession.priceNotes.${pricesBy}`)}</p>
        </Part>

        <Part on={grade} onToggle={setGrade} label={t("newSession.tasks.grade")}>
          <div>
            <label className="label">{t("newSession.gradeWith")}</label>
            <Select value={gradeBy} onChange={setGradeBy} options={engineOptions(false)} />
          </div>
          {local ? (
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
          ) : (
            <div />
          )}
          <div>
            <label className="label">{t("newSession.gradeWhat")}</label>
            <Select value={mode} onChange={setMode} options={gradeModeOptions()} />
          </div>
          {mode === "condition" || (local && mode === "grade") ? (
            <div>
              <label className="label">{t("newSession.conditionScale")}</label>
              <Select
                value={conditionScale(scale, local)}
                onChange={setScale}
                options={local ? localConditionOptions() : conditionOptions()}
              />
            </div>
          ) : (
            <div />
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
