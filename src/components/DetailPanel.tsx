import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import { Check, ExternalLink, FolderOpen, Loader2, RotateCcw, X } from "lucide-react";
import { useState, type ReactNode } from "react";
import type { Json } from "../lib/api";
import type { Candidate, CardView } from "../lib/cardview";
import { basename, grade, price } from "../lib/format";
import StatusChip, { ConfidenceChip, PickedChip } from "./StatusChip";

export default function DetailPanel({
  view,
  onClose,
  onRetry,
  onPick,
  ebay,
}: {
  view: CardView;
  onClose: () => void;
  onRetry?: () => void;
  /** The card's eBay section, when it has a listing. */
  ebay?: ReactNode;
  /** Makes a candidate the match; without it, candidates are read-only. */
  onPick?: (index: number) => Promise<unknown>;
}) {
  return (
    <aside className="flex h-full w-[400px] shrink-0 flex-col border-l border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-950">
      <div className="flex items-start gap-2 border-b border-slate-200 px-4 py-3 dark:border-slate-800">
        <div className="min-w-0 flex-1">
          <h2 className="selectable text-base leading-snug font-semibold">{view.title}</h2>
          <div className="mt-1 flex flex-wrap items-center gap-1.5">
            {view.status && <StatusChip status={view.status} />}
            {view.idType && <span className="text-[11px] font-medium text-slate-500 uppercase">{view.idType}</span>}
            <ConfidenceChip value={view.confidence} />
            {view.picked && <PickedChip />}
            {view.distance != null && (
              <span className="text-[11px] text-slate-400">distance {view.distance.toFixed(3)}</span>
            )}
            {view.via && <span className="text-[11px] text-slate-400">{view.via}</span>}
            {view.local && (
              <span className="text-[11px] text-slate-400">
                {view.local.model}
                {view.local.seconds != null && ` · ${view.local.seconds.toFixed(1)} s`}
              </span>
            )}
          </div>
        </div>
        <button className="btn btn-ghost px-1.5" onClick={onClose} aria-label="Close detail">
          <X className="h-4 w-4" />
        </button>
      </div>

      <div className="min-h-0 flex-1 space-y-5 overflow-y-auto px-4 py-4">
        {view.error && (
          <div className="selectable flex items-start gap-2 rounded-md bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950/50 dark:text-red-300">
            <span className="flex-1">{view.error}</span>
            {onRetry && (
              <button className="btn px-2 py-0.5 text-xs" onClick={onRetry}>
                <RotateCcw className="h-3 w-3" /> Retry
              </button>
            )}
          </div>
        )}

        <Images images={view.images} />

        {(view.price != null || view.grade != null || view.condition) && (
          <div className="grid grid-cols-2 gap-2">
            <Stat label="Market price" value={price(view.price)} />
            <Stat
              label={
                view.gradingEndpoint === "centering"
                  ? "Centering"
                  : view.gradingEndpoint === "condition"
                    ? "Condition"
                    : "Grade"
              }
              value={view.grade != null ? grade(view.grade) : (view.condition ?? "—")}
              sub={view.grade != null ? (view.condition ?? undefined) : undefined}
            />
          </div>
        )}

        {ebay}

        {view.fields.length > 0 && (
          <Section title="Card">
            <dl className="selectable grid grid-cols-[100px_1fr] gap-x-3 gap-y-1.5 text-sm">
              {view.fields.map(([k, v]) => (
                <Field key={k} label={k} value={v} />
              ))}
            </dl>
          </Section>
        )}

        {view.slab && Object.keys(view.slab).length > 0 && (
          <Section title="Slab label">
            <dl className="selectable grid grid-cols-[100px_1fr] gap-x-3 gap-y-1.5 text-sm">
              <Field label="Company" value={view.slab.company} />
              <Field
                label="Grade"
                value={[view.slab.grade ?? view.slab.tagGrade, view.slab.verbal_grade].filter(Boolean).join(" · ")}
              />
              <Field label="Certificate" value={view.slab.certificate_number} />
              <Field label="Label" value={view.slab.name} />
            </dl>
          </Section>
        )}

        <Files images={view.images} />

        <Grading view={view} />

        {view.priceStats.length > 0 && (
          <Section title="Price statistics (USD)">
            <table className="w-full text-xs">
              <thead className="text-slate-500">
                <tr>
                  <th className="py-1 text-left font-medium">Type</th>
                  <th className="py-1 text-right font-medium">Median</th>
                  <th className="py-1 text-right font-medium">Min</th>
                  <th className="py-1 text-right font-medium">Max</th>
                  <th className="py-1 text-right font-medium">Latest</th>
                </tr>
              </thead>
              <tbody className="selectable tabular-nums">
                {view.priceStats.map((p: Json, i: number) => {
                  const v = p.value ?? p;
                  return (
                    <tr key={i} className="border-t border-slate-100 dark:border-slate-800">
                      <td className="py-1 capitalize">{p.stats_type}</td>
                      <td className="py-1 text-right">{price(v.median)}</td>
                      <td className="py-1 text-right">{price(v.min)}</td>
                      <td className="py-1 text-right">{price(v.max)}</td>
                      <td className="py-1 text-right">{price(v.latest)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Section>
        )}

        {Object.keys(view.links).length > 0 && (
          <Section title="Links">
            <div className="flex flex-wrap gap-1.5">
              {Object.entries(view.links).map(([name, url]) => (
                <button key={name} className="btn px-2 py-1 text-xs" onClick={() => openUrl(url)}>
                  {name} <ExternalLink className="h-3 w-3" />
                </button>
              ))}
            </div>
          </Section>
        )}

        {view.alternatives.length > 0 && <Candidates list={view.alternatives} onPick={onPick} />}

        <details className="text-xs">
          <summary className="cursor-pointer font-medium text-slate-500">Raw response</summary>
          <pre className="selectable mt-2 max-h-96 overflow-auto rounded bg-slate-50 p-2 text-[11px] dark:bg-slate-900">
            {JSON.stringify(view.raw, null, 2)}
          </pre>
        </details>
      </div>
    </aside>
  );
}

function Images({ images }: { images: CardView["images"] }) {
  const [active, setActive] = useState(0);
  const img = images[Math.min(active, images.length - 1)];
  if (!img) return null;
  return (
    <div>
      <div className="flex h-72 items-center justify-center rounded-lg bg-slate-100 p-2 dark:bg-slate-900">
        {img.src ? (
          <img
            src={img.src}
            alt={img.label}
            className={clsx("max-h-full max-w-full rounded object-contain shadow", img.path && "cursor-zoom-in")}
            onDoubleClick={() => img.path && openPath(img.path)}
            title={img.path ? "Double-click to open the original" : undefined}
          />
        ) : (
          <span className="text-sm text-slate-400">No image</span>
        )}
      </div>
      {images.length > 1 && (
        <div className="mt-2 flex justify-center gap-1">
          {images.map((im, i) => (
            <button
              key={im.label + i}
              onClick={() => setActive(i)}
              className={clsx(
                "rounded px-2 py-0.5 text-xs",
                i === active ? "bg-accent text-white" : "btn-ghost text-slate-500",
              )}
            >
              {im.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** The scanned files the card was read from: name, folder, and a way to find them. */
function Files({ images }: { images: CardView["images"] }) {
  const files = images.filter((im) => im.path);
  if (!files.length) return null;
  return (
    <Section title="Files">
      <dl className="grid grid-cols-[100px_1fr_auto] items-start gap-x-3 gap-y-1.5 text-sm">
        {files.map(({ label, path }) => (
          <div key={label} className="contents">
            <dt className="text-slate-500">{label}</dt>
            <dd className="selectable min-w-0" title={path}>
              <span className="block break-all">{basename(path!)}</span>
              <span className="block text-xs break-all text-slate-400">{folderOf(path!)}</span>
            </dd>
            <button
              className="btn btn-ghost px-1.5 py-0.5"
              onClick={() => revealItemInDir(path!)}
              title="Show in folder"
              aria-label={`Show the ${label.toLowerCase()} file in its folder`}
            >
              <FolderOpen className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}
      </dl>
    </Section>
  );
}

function folderOf(path: string): string {
  const name = basename(path);
  return path.endsWith(name) ? path.slice(0, path.length - name.length).replace(/[\\/]+$/, "") : "";
}

const SUBGRADES = ["corners", "edges", "surface", "centering"] as const;

function Grading({ view }: { view: CardView }) {
  if (!view.gradingEndpoint) return null;
  const records = view.gradingRecords;
  const grades: Json = records.find((r: Json) => r?.grades)?.grades ?? {};
  const sides = records.filter((r: Json) => r?.grades);
  const centering = records[0]?.card?.[0]?.centering;
  const condition = records[0]?._objects?.[0]?.Condition?.[0];

  return (
    <Section title={`Grading · ${view.gradingEndpoint === "imported" ? "from CSV" : view.gradingEndpoint}`}>
      {condition && (
        <p className="text-sm">
          <span className="font-semibold">{condition.label}</span>
          <span className="text-slate-500">
            {" "}
            ({condition.mode} scale{condition.value != null ? `, ${Number(condition.value).toFixed(2)}` : ""})
          </span>
        </p>
      )}
      {SUBGRADES.some((k) => grades[k] != null) && (
        <div className="space-y-1.5">
          {SUBGRADES.filter((k) => grades[k] != null).map((k) => (
            <div key={k} className="flex items-center gap-2 text-sm">
              <span className="w-20 text-slate-500 capitalize">{k}</span>
              <div className="h-2 flex-1 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
                <div className="h-full rounded-full bg-violet-500" style={{ width: `${(grades[k] / 10) * 100}%` }} />
              </div>
              <span className="w-8 text-right tabular-nums">{grade(grades[k])}</span>
            </div>
          ))}
        </div>
      )}
      {sides.length > 1 && (
        <p className="mt-2 text-xs text-slate-500">
          {sides.map((r: Json, i: number) => `${i === 0 ? "Front" : "Back"} ${grade(r.grades.final)}`).join(" · ")}
        </p>
      )}
      {centering && (
        <p className="mt-2 text-xs text-slate-500">
          Centering L/R {centering["left/right"]} · T/B {centering["top/bottom"]}
        </p>
      )}
      {view.gradingImages.length > 0 && (
        <div className="mt-3 grid grid-cols-2 gap-2">
          {view.gradingImages.map((g) => (
            <button
              key={g.path ?? g.src}
              onClick={() => g.path && openPath(g.path)}
              className="overflow-hidden rounded border border-slate-200 dark:border-slate-800"
              title={`${g.label} — click to open`}
            >
              <img src={g.src} alt={g.label} className="h-32 w-full bg-slate-100 object-contain dark:bg-slate-900" />
            </button>
          ))}
        </div>
      )}
    </Section>
  );
}

function Candidates({ list, onPick }: { list: Candidate[]; onPick?: (index: number) => Promise<unknown> }) {
  const [busy, setBusy] = useState<number | null>(null);
  const showPrice = list.some((c) => c.price != null);
  const pick = (index: number) => {
    if (!onPick) return;
    setBusy(index);
    onPick(index).finally(() => setBusy(null));
  };
  return (
    <Section title={`Other candidates (${list.length})`}>
      <div className="divide-y divide-slate-100 overflow-hidden rounded-lg border border-slate-200 dark:divide-slate-800 dark:border-slate-800">
        {list.map((c) => (
          <div key={`${c.index}:${c.fullName}`} className="flex items-center gap-2.5 px-2.5 py-1.5 text-sm">
            <div className="selectable min-w-0 flex-1" title={c.fullName || undefined}>
              <div className="truncate font-medium">{c.name}</div>
              {c.detail && <div className="truncate text-xs text-slate-500">{c.detail}</div>}
            </div>
            <div
              className="flex shrink-0 flex-col items-end gap-0.5"
              title={c.distance != null ? `distance ${c.distance.toFixed(3)}` : undefined}
            >
              {showPrice && <span className="text-xs tabular-nums">{price(c.price)}</span>}
              <ConfidenceChip value={c.confidence} />
            </div>
            {onPick && (
              <button
                className="btn px-2 py-0.5 text-xs"
                disabled={busy != null}
                onClick={() => pick(c.index)}
                title="Use this card as the match"
              >
                {busy === c.index ? <Loader2 className="h-3 w-3 animate-spin" /> : <Check className="h-3 w-3" />}
                Select
              </button>
            )}
          </div>
        ))}
      </div>
    </Section>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase">{title}</h3>
      {children}
    </section>
  );
}

function Field({ label, value }: { label: string; value?: string | null }) {
  if (!value) return null;
  return (
    <>
      <dt className="text-slate-500">{label}</dt>
      <dd className="min-w-0 break-words">{value}</dd>
    </>
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
