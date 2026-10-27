// On-device models: the list with download progress, kept live from `model-updated` events.

import { openUrl } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import { Cpu, Download, ExternalLink, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { rich, t as translate, useT } from "../i18n";
import { api, errorText, onModelUpdated, type ModelStatus } from "../lib/api";
import { bytes, duration } from "../lib/format";
import { ErrorNote } from "./Modal";
import Select, { Badge, type SelectOption } from "./Select";

/** The models and their download state, updated as downloads progress. */
export function useModels() {
  const [models, setModels] = useState<ModelStatus[] | null>(null);
  useEffect(() => {
    let alive = true;
    api.listModels().then((m) => alive && setModels(m));
    const unlisten = onModelUpdated((m) => setModels((list) => list && list.map((x) => (x.id === m.id ? m : x))));
    return () => {
      alive = false;
      void unlisten.then((f) => f());
    };
  }, []);
  return models;
}

/** Download speed from recent progress samples: bytes per second. */
function useSpeed(model: ModelStatus) {
  const samples = useRef<[number, number][]>([]);
  const [speed, setSpeed] = useState<number | null>(null);
  useEffect(() => {
    if (model.state !== "downloading") {
      samples.current = [];
      return;
    }
    const now = Date.now();
    samples.current = [...samples.current.filter(([t]) => now - t < 8000), [now, model.downloaded]];
    const [t0, b0] = samples.current[0];
    const rate = (model.downloaded - b0) / ((now - t0) / 1000);
    const t = setTimeout(() => setSpeed(Number.isFinite(rate) && rate > 0 ? rate : null), 0);
    return () => clearTimeout(t);
  }, [model.downloaded, model.state]);
  return model.state === "downloading" ? speed : null;
}

const TIER_STYLES: Record<string, string> = {
  Fast: "bg-sky-50 text-sky-700 dark:bg-sky-950 dark:text-sky-300",
  Balanced: "bg-violet-50 text-violet-700 dark:bg-violet-950 dark:text-violet-300",
  Accurate: "bg-amber-50 text-amber-700 dark:bg-amber-950 dark:text-amber-300",
};

/** The model's Hugging Face repository, e.g. `LiquidAI/LFM2.5-VL-450M-GGUF`. */
const repoOf = (m: ModelStatus) => m.homepage.replace(/^https:\/\/huggingface\.co\//, "");

/** A model as a select option: repository and parameters, and a green badge once downloaded. */
export function modelOption(m: ModelStatus): SelectOption<string> {
  return {
    value: m.id,
    text: repoOf(m),
    label: (
      <span className="flex items-center gap-2">
        <span className="truncate font-mono text-[13px]">{repoOf(m)}</span>
        <Badge>{m.params}</Badge>
      </span>
    ),
    badge:
      m.state === "ready" ? (
        <Badge tone="green">{translate("models.picker.downloaded")}</Badge>
      ) : m.state === "downloading" ? (
        <Badge tone="sky">{m.size ? Math.floor((m.downloaded / m.size) * 100) : 0}%</Badge>
      ) : undefined,
  };
}

/** A select of the models (with their download state), and the chosen one's details and download button. */
export function ModelPicker({
  models,
  value,
  onChange,
}: {
  models: ModelStatus[];
  value: string;
  onChange: (id: string) => void;
}) {
  const t = useT();
  const selected = models.find((m) => m.id === value) ?? models[0];
  if (!selected) return null;
  return (
    <div className="space-y-3">
      <div>
        <div className="label">{t("models.picker.model")}</div>
        <Select
          value={selected.id}
          options={models.map(modelOption)}
          onChange={onChange}
          aria-label={t("models.picker.model")}
        />
      </div>
      {/* Keyed: a license accepted for one model isn't accepted for the next. */}
      <ModelCard key={selected.id} model={selected} />
    </div>
  );
}

/** One model: what it is, and downloading or deleting it. */
function ModelCard({ model }: { model: ModelStatus }) {
  const t = useT();
  const [accepted, setAccepted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const speed = useSpeed(model);
  const act = (f: () => Promise<unknown>) => {
    setError(null);
    f().catch((e) => setError(errorText(e)));
  };
  const pct = model.size ? Math.min(100, (model.downloaded / model.size) * 100) : 0;
  const left = speed ? (model.size - model.downloaded) / speed : null;

  return (
    <div className="rounded-lg border border-slate-200 p-4 dark:border-slate-800">
      <div className="flex items-start gap-3">
        <span className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent/10 text-accent">
          <Cpu className="h-5 w-5" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-medium">{model.name}</span>
            <span className={clsx("rounded px-1.5 py-0.5 text-[11px] font-medium", TIER_STYLES[model.tier])}>
              {model.tier}
            </span>
            <span className="text-xs text-slate-500">
              {t("models.card.sizes", { size: bytes(model.size), memory: bytes(model.memoryMb * 1e6) })}
            </span>
          </div>
          <p className="mt-0.5 text-sm text-slate-500">{model.description}</p>
          <button
            className="mt-1 inline-flex items-center gap-1 text-xs text-accent hover:underline"
            onClick={() => openUrl(model.homepage)}
          >
            {t("models.card.modelPage")} <ExternalLink className="h-3 w-3" />
          </button>
        </div>
      </div>

      {!model.supported ? (
        <p className="mt-3 text-sm text-amber-700 dark:text-amber-400">{t("models.card.unsupported")}</p>
      ) : model.state === "downloading" ? (
        <div className="mt-3 space-y-1.5">
          <div className="h-2 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
            <div
              className="progress-stripes h-full rounded-full bg-accent transition-all"
              style={{ width: `${pct}%` }}
            />
          </div>
          <div className="flex items-center gap-3 text-xs text-slate-500">
            <span className="tabular-nums">
              {t("models.card.progress", {
                done: bytes(model.downloaded),
                size: bytes(model.size),
                percent: Math.floor(pct),
              })}
            </span>
            {speed && <span className="tabular-nums">{bytes(speed)}/s</span>}
            {left != null && <span>{t("models.card.left", { time: duration(left) })}</span>}
            <span className="flex-1" />
            <button className="btn px-2 py-0.5 text-xs" onClick={() => act(() => api.cancelModelDownload(model.id))}>
              <X className="h-3 w-3" /> {t("common.cancel")}
            </button>
          </div>
        </div>
      ) : model.state === "ready" ? (
        <div className="mt-3 flex items-center gap-3 text-xs text-slate-500">
          <span>{t("models.card.ready")}</span>
          <span className="flex-1" />
          <button className="btn px-2 py-0.5 text-xs" onClick={() => act(() => api.deleteModel(model.id))}>
            <Trash2 className="h-3 w-3" /> {t("common.delete")}
          </button>
        </div>
      ) : (
        <div className="mt-3 space-y-2">
          <label className="flex cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              className="h-4 w-4 accent-[var(--color-accent)]"
              checked={accepted}
              onChange={(e) => setAccepted(e.target.checked)}
            />
            <span>
              {rich(t("models.card.accept", { license: model.licenseName }), {
                // A link inside the label: opening the license doesn't tick the box.
                license: (s) => (
                  <button
                    className="text-accent hover:underline"
                    onClick={(e) => {
                      e.preventDefault();
                      void openUrl(model.licenseUrl);
                    }}
                  >
                    {s}
                  </button>
                ),
              })}
            </span>
          </label>
          <p className="text-xs text-slate-500">{t("models.card.licenseNote")}</p>
          <div className="flex items-center gap-3">
            <button
              className="btn btn-primary"
              disabled={!accepted}
              onClick={() => act(() => api.downloadModel(model.id))}
            >
              <Download className="h-4 w-4" />{" "}
              {model.downloaded > 0 ? t("models.card.resume") : t("models.card.download")}
            </button>
            {model.downloaded > 0 && (
              <span className="text-xs text-slate-500">
                {t("models.card.partial", { done: bytes(model.downloaded), size: bytes(model.size) })}
              </span>
            )}
          </div>
        </div>
      )}
      {(model.error || error) && (
        <div className="mt-3">
          <ErrorNote>{model.error ?? error}</ErrorNote>
        </div>
      )}
    </div>
  );
}
