// "Compare with the local model": runs the on-device model on the cards Ximilar identified in a session and
// shows how often the two agree. The session is not changed.

import clsx from "clsx";
import { CheckCircle2, XCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { api, errorText, onCompareProgress, type CompareReport, type CompareRow, type Session } from "../lib/api";
import { decimal } from "../lib/format";
import { modelOption, useModels } from "./LocalModels";
import Modal, { ErrorNote } from "./Modal";
import Select from "./Select";

interface Result {
  model: string;
  names: string;
  numbers: string;
  seconds: string;
}

export default function CompareDialog({ session, onClose }: { session: Session; onClose: () => void }) {
  const t = useT();
  const models = useModels();
  const ready = models?.filter((m) => m.state === "ready") ?? [];
  const [picked, setPicked] = useState<string | null>(null);
  const [run, setRun] = useState<{ model: string; n: number } | null>(null);
  const [results, setResults] = useState<Result[]>([]);
  const [running, setRunning] = useState(false);
  const model =
    picked ?? (ready.some((m) => m.id === session.options.localModel) ? session.options.localModel : ready[0]?.id);
  const nameOf = (id: string) => models?.find((m) => m.id === id)?.name ?? id;

  return (
    <Modal
      title={t("models.compare.title")}
      onClose={onClose}
      footer={
        <button className="btn" onClick={onClose}>
          {running ? t("models.compare.stop") : t("common.close")}
        </button>
      }
    >
      <div className="space-y-4">
        <p className="text-sm text-slate-600 dark:text-slate-300">
          {t("models.compare.intro", { name: session.name })}
        </p>
        {models && ready.length === 0 ? (
          <ErrorNote>{t("models.compare.downloadFirst")}</ErrorNote>
        ) : (
          <div className="flex items-end gap-2">
            <div className="flex-1">
              <label className="label">{t("models.compare.model")}</label>
              <Select
                value={model ?? ""}
                onChange={setPicked}
                disabled={running}
                options={ready.map(modelOption)}
                aria-label={t("models.compare.model")}
              />
            </div>
            <button
              className="btn btn-primary"
              disabled={!model || running}
              onClick={() => model && setRun((r) => ({ model, n: (r?.n ?? 0) + 1 }))}
            >
              {results.length ? t("models.compare.compareAgain") : t("models.compare.compare")}
            </button>
          </div>
        )}
        {results.length > 1 && (
          <table className="w-full text-sm">
            <thead className="text-left text-xs text-slate-500">
              <tr>
                <th className="py-1 font-medium">{t("models.compare.model")}</th>
                <th className="py-1 text-right font-medium">{t("models.compare.sameName")}</th>
                <th className="py-1 text-right font-medium">{t("models.compare.sameNumber")}</th>
                <th className="py-1 text-right font-medium">{t("models.compare.perCard")}</th>
              </tr>
            </thead>
            <tbody className="tabular-nums">
              {results.map((r, i) => (
                <tr key={i} className="border-t border-slate-100 dark:border-slate-800">
                  <td className="py-1">{nameOf(r.model)}</td>
                  <td className="py-1 text-right">{r.names}</td>
                  <td className="py-1 text-right">{r.numbers}</td>
                  <td className="py-1 text-right">{r.seconds}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {run && (
          <CompareRun
            key={run.n}
            session={session}
            model={run.model}
            onRunning={setRunning}
            onDone={(r) => setResults((list) => [...list, r])}
          />
        )}
      </div>
    </Modal>
  );
}

const pct = (n: number, of: number) => (of ? `${Math.round((n / of) * 100)}%` : "—");

function CompareRun({
  session,
  model,
  onRunning,
  onDone,
}: {
  session: Session;
  model: string;
  onRunning: (running: boolean) => void;
  onDone: (r: Result) => void;
}) {
  const t = useT();
  const [rows, setRows] = useState<CompareRow[]>([]);
  const [total, setTotal] = useState<number | null>(null);
  const [report, setReport] = useState<CompareReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    const runId = crypto.randomUUID();
    const unlisten = onCompareProgress((p) => {
      if (!alive || p.runId !== runId) return;
      setTotal(p.total);
      setRows((r) => [...r, p.row]);
    });
    onRunning(true);
    api
      .compareLocal(session.id, runId, model)
      .then((r) => {
        if (!alive) return;
        setReport(r);
        onDone({
          model,
          names: pct(r.names, r.cards),
          numbers: pct(r.numbers, r.withNumber),
          seconds: `${decimal(r.secondsPerCard, 1)} s`,
        });
      })
      .catch((e) => alive && setError(errorText(e)))
      .finally(() => alive && onRunning(false));
    return () => {
      alive = false;
      onRunning(false);
      void api.cancelCompare(runId);
      void unlisten.then((f) => f());
    };
    // Each run is its own component instance (keyed); the callbacks don't restart it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session.id, model]);

  const done = report?.rows ?? rows;
  const names = done.filter((r) => r.nameMatches).length;
  const numbers = done.filter((r) => r.numberMatches).length;
  const withNumber = report?.withNumber;
  const running = !report && !error;

  return (
    <div className="space-y-4">
      <ErrorNote>{error}</ErrorNote>
      {running && (
        <div className="space-y-1">
          <div className="h-2 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
            <div
              className="progress-stripes h-full rounded-full bg-accent transition-all"
              style={{ width: total ? `${(rows.length / total) * 100}%` : "5%" }}
            />
          </div>
          <p className="text-xs text-slate-500 tabular-nums">
            {total == null
              ? t("models.compare.loadingModel")
              : t("models.compare.progress", { done: rows.length, count: total })}
          </p>
        </div>
      )}
      {done.length > 0 && (
        <div className="grid grid-cols-3 gap-2">
          <Stat
            label={t("models.compare.sameName")}
            value={pct(names, done.length)}
            sub={t("models.compare.of", { n: names, total: done.length })}
          />
          <Stat
            label={t("models.compare.sameNumber")}
            value={withNumber != null ? pct(numbers, withNumber) : "…"}
            sub={withNumber != null ? t("models.compare.ofWithNumber", { n: numbers, total: withNumber }) : undefined}
          />
          <Stat
            label={t("models.compare.perCard")}
            value={`${decimal(done.reduce((a, r) => a + r.seconds, 0) / done.length, 1)} s`}
            sub={t("models.compare.onThisComputer")}
          />
        </div>
      )}
      {done.length > 0 && (
        <div className="max-h-80 overflow-y-auto rounded-lg border border-slate-200 dark:border-slate-800">
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-slate-50 text-left text-xs text-slate-500 dark:bg-slate-900">
              <tr>
                <th className="px-3 py-1.5 font-medium">#</th>
                <th className="px-3 py-1.5 font-medium">Ximilar</th>
                <th className="px-3 py-1.5 font-medium">{t("models.compare.localModel")}</th>
                <th className="px-3 py-1.5 font-medium">{t("models.compare.name")}</th>
                <th className="px-3 py-1.5 font-medium">{t("models.compare.number")}</th>
              </tr>
            </thead>
            <tbody className="selectable">
              {done.map((r) => (
                <tr key={r.itemId} className="border-t border-slate-100 dark:border-slate-800">
                  <td className="px-3 py-1.5 text-xs text-slate-400 tabular-nums">{r.seq}</td>
                  <td className="max-w-56 truncate px-3 py-1.5" title={r.ximilar}>
                    {r.ximilar}
                  </td>
                  <td
                    className={clsx("max-w-56 truncate px-3 py-1.5", r.error && "text-red-600")}
                    title={r.error ?? r.local}
                  >
                    {r.error ?? (r.local || <span className="text-slate-400">{t("models.compare.nothingRead")}</span>)}
                  </td>
                  <td className="px-3 py-1.5">
                    <Mark ok={r.nameMatches} />
                  </td>
                  <td className="px-3 py-1.5">
                    <Mark ok={r.numberMatches} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {report && report.cards === 0 && <p className="text-sm text-slate-500">{t("models.compare.noCards")}</p>}
    </div>
  );
}

function Mark({ ok }: { ok: boolean }) {
  const t = useT();
  return ok ? (
    <CheckCircle2 className="h-4 w-4 text-emerald-600" aria-label={t("models.compare.same")} />
  ) : (
    <XCircle className="h-4 w-4 text-slate-300 dark:text-slate-600" aria-label={t("models.compare.different")} />
  );
}

function Stat({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="rounded-lg bg-slate-50 px-3 py-2 dark:bg-slate-900">
      <div className="text-[11px] font-medium tracking-wide text-slate-500 uppercase">{label}</div>
      <div className="text-xl font-semibold tabular-nums">{value}</div>
      {sub && <div className="text-xs text-slate-500">{sub}</div>}
    </div>
  );
}
