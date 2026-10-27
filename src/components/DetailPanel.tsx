import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import {
  CircleCheck,
  CircleQuestionMark,
  Check,
  ChevronDown,
  ChevronUp,
  ExternalLink,
  FolderOpen,
  Loader2,
  Pencil,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { errorText, type ItemEdit, type Json } from "../lib/api";
import type { Candidate, CardView } from "../lib/cardview";
import { basename, grade, price } from "../lib/format";
import { ErrorNote } from "./Modal";
import StatusChip, { AcceptedChip, ConfidenceChip, EditedChip, PickedChip, ReviewChip } from "./StatusChip";

export default function DetailPanel({
  view,
  onClose,
  onRetry,
  onPick,
  onAccept,
  onUnaccept,
  onEdit,
  onDelete,
  onPrev,
  onNext,
  position,
  ebay,
  extra,
}: {
  view: CardView;
  onClose: () => void;
  onRetry?: () => void;
  /** The card's eBay section, when it has a listing. */
  ebay?: ReactNode;
  /** Makes a candidate the match; without it, candidates are read-only. */
  onPick?: (index: number) => Promise<unknown>;
  /** Accepts the match of a card to review. */
  onAccept?: () => void;
  /** Takes an accept back: the card is to review again (if its match is unsure). */
  onUnaccept?: () => void;
  /** Saves what the user typed; without it, the card can't be edited. */
  onEdit?: (edit: ItemEdit) => Promise<unknown>;
  onDelete?: () => void;
  onPrev?: () => void;
  onNext?: () => void;
  /** "3 of 120" */
  position?: string;
  /** Shown under the image, e.g. a link to the card's session. */
  extra?: ReactNode;
}) {
  const [editing, setEditing] = useState(false);

  // Reviewing from the keyboard: A accepts, E edits, 1–9 pick a candidate, Delete deletes, Escape closes.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || editing) return;
      if ((e.target as HTMLElement)?.closest("input, textarea, select") || document.querySelector("[role=dialog]"))
        return;
      const candidate = /^[1-9]$/.test(e.key) ? view.alternatives[Number(e.key) - 1] : undefined;
      if (e.key === "Escape") onClose();
      else if (e.key === "a" && view.accepted && onUnaccept) onUnaccept();
      else if (e.key === "a" && !view.accepted && onAccept) onAccept();
      else if (e.key === "e" && onEdit && view.edit) setEditing(true);
      else if ((e.key === "Delete" || e.key === "Backspace") && onDelete) onDelete();
      else if (candidate && onPick) void onPick(candidate.index);
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, editing, onClose, onAccept, onUnaccept, onEdit, onDelete, onPick]);

  return (
    <aside className="flex h-full w-[400px] shrink-0 flex-col border-l border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-950">
      <div className="flex items-start gap-2 border-b border-slate-200 px-4 py-3 dark:border-slate-800">
        <div className="min-w-0 flex-1">
          <h2 className="selectable text-base leading-snug font-semibold">{view.title}</h2>
          <div className="mt-1 flex flex-wrap items-center gap-1.5">
            {view.status && <StatusChip status={view.status} />}
            {view.idType && <span className="text-[11px] font-medium text-slate-500 uppercase">{view.idType}</span>}
            <ConfidenceChip value={view.confidence} />
            {view.picked && !view.edited && <PickedChip />}
            {view.edited && <EditedChip />}
            {view.accepted && <AcceptedChip />}
            {view.toReview && <ReviewChip />}
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
        {(onPrev || onNext) && (
          <div className="flex items-center">
            <button
              className="btn btn-ghost px-1.5"
              onClick={onPrev}
              disabled={!onPrev}
              title="Previous card (K or ↑)"
              aria-label="Previous card"
            >
              <ChevronUp className="h-4 w-4" />
            </button>
            <button
              className="btn btn-ghost px-1.5"
              onClick={onNext}
              disabled={!onNext}
              title="Next card (J or ↓)"
              aria-label="Next card"
            >
              <ChevronDown className="h-4 w-4" />
            </button>
          </div>
        )}
        <button className="btn btn-ghost px-1.5" onClick={onClose} aria-label="Close detail" title="Close (Esc)">
          <X className="h-4 w-4" />
        </button>
      </div>
      {(view.toReview && onAccept) || position ? (
        <div
          className={clsx(
            "flex items-center gap-2 border-b border-slate-200 px-4 py-2 text-xs dark:border-slate-800",
            view.toReview && onAccept && "bg-orange-50/70 dark:bg-orange-950/20",
            view.accepted && "bg-emerald-50/60 dark:bg-emerald-950/20",
          )}
        >
          {view.toReview && onAccept ? (
            <>
              <span className="flex flex-1 items-center gap-1.5 text-orange-800 dark:text-orange-300">
                <CircleQuestionMark className="h-4 w-4 shrink-0 text-orange-500" strokeWidth={2.5} />
                Unsure match — accept it or pick another candidate below.
              </span>
              <button className="btn btn-primary px-2 py-0.5 text-xs" onClick={onAccept} title="Accept the match (A)">
                <Check className="h-3 w-3" /> Accept
              </button>
            </>
          ) : view.accepted ? (
            <>
              <span className="flex flex-1 items-center gap-1.5 text-emerald-800 dark:text-emerald-300">
                <CircleCheck className="h-4 w-4 shrink-0 text-emerald-600" strokeWidth={2.5} />
                You accepted this match{position && ` · ${position}`}
              </span>
              {onUnaccept && (
                <button className="btn px-2 py-0.5 text-xs" onClick={onUnaccept} title="Take the accept back (A)">
                  Unaccept
                </button>
              )}
            </>
          ) : (
            <span className="flex-1 text-slate-500">{position}</span>
          )}
        </div>
      ) : null}

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

        {extra}
        {ebay}

        {editing && view.edit && onEdit ? (
          <EditForm
            initial={view.edit}
            identified={view.status === "done" || view.status === "grading"}
            onCancel={() => setEditing(false)}
            onSave={(edit) => onEdit(edit).then(() => setEditing(false))}
          />
        ) : (
          (view.fields.length > 0 || view.edit?.notes || view.edit?.location || onEdit) && (
            <Section
              title="Card"
              action={
                onEdit &&
                view.edit && (
                  <button
                    className="btn btn-ghost px-1.5 py-0.5 text-xs"
                    onClick={() => setEditing(true)}
                    title="Edit (E)"
                  >
                    <Pencil className="h-3 w-3" /> Edit
                  </button>
                )
              }
            >
              <dl className="selectable grid grid-cols-[100px_1fr] gap-x-3 gap-y-1.5 text-sm">
                {view.fields.map(([k, v]) => (
                  <Field key={k} label={k} value={v} />
                ))}
                <Field label="Location" value={view.edit?.location} />
                <Field label="Notes" value={view.edit?.notes} />
              </dl>
            </Section>
          )
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

        {onDelete && (
          <button
            className="btn btn-ghost w-full justify-center text-xs text-red-600 dark:text-red-400"
            onClick={onDelete}
          >
            <Trash2 className="h-3.5 w-3.5" /> Delete this card
          </button>
        )}
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
        {list.map((c, i) => (
          <div key={`${c.index}:${c.fullName}`} className="flex items-center gap-2.5 px-2.5 py-1.5 text-sm">
            {onPick && i < 9 && (
              <kbd className="w-4 shrink-0 text-center text-[11px] text-slate-400" title={`Press ${i + 1} to select`}>
                {i + 1}
              </kbd>
            )}
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

function Section({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-xs font-semibold tracking-wide text-slate-500 uppercase">{title}</h3>
        {action}
      </div>
      {children}
    </section>
  );
}

const EDIT_FIELDS: [keyof ItemEdit, string][] = [
  ["name", "Name"],
  ["setName", "Set"],
  ["setCode", "Set code"],
  ["cardNumber", "Number"],
  ["year", "Year"],
];

/** The card's fields, price, location and notes as a form; Enter saves, Escape cancels. */
function EditForm({
  initial,
  identified,
  onSave,
  onCancel,
}: {
  initial: ItemEdit;
  /** Card fields and price can be typed only once the card is identified. */
  identified: boolean;
  onSave: (edit: ItemEdit) => Promise<unknown>;
  onCancel: () => void;
}) {
  const [edit, setEdit] = useState(initial);
  const [priceText, setPriceText] = useState(initial.price != null ? String(initial.price) : "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (k: keyof ItemEdit, v: string) => setEdit((e) => ({ ...e, [k]: v }));

  const submit = () => {
    const text = priceText.trim().replace(",", ".").replace(/^\$/, "");
    const value = text === "" ? null : Number(text);
    if (value != null && (!Number.isFinite(value) || value < 0))
      return setError("The price must be a positive number.");
    setBusy(true);
    setError(null);
    onSave({ ...edit, price: value }).catch((e) => {
      setError(errorText(e));
      setBusy(false);
    });
  };

  const input = (label: string, value: string, onChange: (v: string) => void, disabled = false, autoFocus = false) => (
    <label key={label} className="contents">
      <span className="pt-1.5 text-slate-500">{label}</span>
      <input
        className="input py-1"
        value={value}
        disabled={disabled}
        autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );

  return (
    <Section title="Edit card">
      <form
        className="grid grid-cols-[80px_1fr] gap-x-3 gap-y-1.5 text-sm"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            onCancel();
          }
        }}
      >
        {EDIT_FIELDS.map(([k, label], i) =>
          input(label, String(edit[k] ?? ""), (v) => set(k, v), !identified, identified && i === 0),
        )}
        {input("Price $", priceText, setPriceText, !identified)}
        {input("Location", edit.location, (v) => set("location", v), false, !identified)}
        <label className="contents">
          <span className="pt-1.5 text-slate-500">Notes</span>
          <textarea className="input min-h-14 py-1" value={edit.notes} onChange={(e) => set("notes", e.target.value)} />
        </label>
        <div className="col-span-2 mt-1 space-y-2">
          {!identified && (
            <p className="text-xs text-slate-500">The card fields can be edited once it is identified.</p>
          )}
          <ErrorNote>{error}</ErrorNote>
          <div className="flex justify-end gap-2">
            <button type="button" className="btn" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy}>
              Save
            </button>
          </div>
        </div>
      </form>
    </Section>
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
