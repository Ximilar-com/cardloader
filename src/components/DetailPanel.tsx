import { openFile, openLink, reveal } from "../lib/open";
import clsx from "clsx";
import {
  Bot,
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
  RotateCw,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { t, useT, type Key } from "../i18n";
import { api, errorText, MAX_TITLE, type EbayMarket, type ItemEdit, type Json, type TitlePreview } from "../lib/api";
import type { Candidate, CardView } from "../lib/cardview";
import { basename, dateTime, decimal, grade, parseAmount, price } from "../lib/format";
import { ebaySite, ebaySiteOptions } from "../lib/ebaySites";
import { ErrorNote, useBackdropClose } from "./Modal";
import Select from "./Select";
import StatusChip, { AcceptedChip, AgentChip, ConfidenceChip, EditedChip, PickedChip, ReviewChip } from "./StatusChip";
import { CustomTitleMark, leftOutText, TitleCount, titleLength } from "./TitleParts";

export default function DetailPanel({
  view,
  onClose,
  onRetry,
  onPick,
  onAccept,
  onUnaccept,
  onEdit,
  listing,
  onDelete,
  onTurn,
  onPrev,
  onNext,
  position,
  ebay,
  extra,
  cropping,
  onFindListings,
  ebaySite,
  onKeepListings,
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
  /** Takes an accept back: the card is to review again. */
  onUnaccept?: () => void;
  /** Saves what the user typed; without it, the card can't be edited. */
  onEdit?: (edit: ItemEdit) => Promise<unknown>;
  /** An identified local card: its listing title is shown and can be written. `rev` changes when it is saved. */
  listing?: { id: number; rev: string };
  onDelete?: () => void;
  /** Turns the card's image upright (it was upside down). */
  onTurn?: () => Promise<unknown>;
  onPrev?: () => void;
  onNext?: () => void;
  /** "3 of 120" */
  position?: string;
  /** Shown under the image, e.g. a link to the card's session. */
  extra?: ReactNode;
  /** The card is a crop session's. */
  cropping?: boolean;
  /** Searches eBay for the card's active listings; without it, only listings an agent found show. */
  onFindListings?: (marketplace: string) => Promise<unknown>;
  /** The eBay site a search starts on (Settings' search site). */
  ebaySite?: string;
  /** Only the listings with these ids are the card: with it, each listing has a tick. */
  onKeepListings?: (ids: string[]) => Promise<unknown>;
}) {
  const t = useT();
  const [editing, setEditing] = useState(false);
  /** The picture shown (front, back), and whether it is enlarged over the app. */
  const [side, setSide] = useState(0);
  const [zoomed, setZoomed] = useState(false);
  const [turning, setTurning] = useState(false);
  const [turnBusy, setTurnBusy] = useState(false);
  const [titled, setTitled] = useState<TitlePreview | null>(null);
  const [listingId, listingRev] = [listing?.id, listing?.rev];
  useEffect(() => {
    if (listingId == null) return;
    let live = true;
    api
      .previewTitles({ itemIds: [listingId] })
      .then((titles) => live && setTitled(titles[0] ?? null))
      .catch(() => live && setTitled(null));
    return () => {
      live = false;
    };
  }, [listingId, listingRev]);
  // A card that can't be listed (a comic) has no title to show or write.
  const title = listing && titled?.itemId === listing.id && !titled.problem ? titled : null;
  // Only upside down is certain to be fixed by turning the picture half way.
  const canTurn = !!onTurn && view.rotation?.tag === "rotated_upside_down";

  // Reviewing from the keyboard: A accepts, E edits, 1–9 pick a candidate, Space enlarges the picture, Delete
  // deletes, Escape closes.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // A key something else took (Escape closing an open menu) isn't for the panel.
      if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || editing) return;
      if ((e.target as HTMLElement)?.closest("input, textarea, select") || document.querySelector("[role=dialog]"))
        return;
      const candidate = /^[1-9]$/.test(e.key) ? view.alternatives[Number(e.key) - 1] : undefined;
      if (e.key === "Escape") onClose();
      // A card an agent reviewed: A makes the review the user's own.
      else if (e.key === "a" && view.byAgent && onAccept) onAccept();
      else if (e.key === "a" && view.accepted && onUnaccept) onUnaccept();
      else if (e.key === "a" && !view.accepted && onAccept) onAccept();
      else if (e.key === "e" && onEdit && view.edit) setEditing(true);
      else if (e.key === " " && view.images.some((im) => im.src)) setZoomed(true);
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
            {view.status && <StatusChip status={view.status} cropping={cropping} />}
            {view.idType && <span className="text-[11px] font-medium text-slate-500 uppercase">{view.idType}</span>}
            <ConfidenceChip value={view.confidence} />
            {view.picked && !view.edited && <PickedChip />}
            {view.edited && <EditedChip />}
            {view.byAgent ? <AgentChip note={view.reviewNote} /> : view.accepted && <AcceptedChip />}
            {view.toReview && <ReviewChip />}
            {view.rotation && (
              <button
                className={clsx(
                  "inline-flex items-center gap-1 rounded bg-violet-50 px-1.5 py-0.5 text-[11px] font-medium text-violet-700 dark:bg-violet-950 dark:text-violet-300",
                  canTurn ? "hover:bg-violet-100 dark:hover:bg-violet-900" : "cursor-default",
                )}
                onClick={() => canTurn && setTurning(!turning)}
                title={
                  (view.rotation.prob != null
                    ? t("detail.rotationTitleProb", {
                        tag: view.rotation.tag,
                        prob: Math.round(view.rotation.prob * 100),
                      })
                    : t("detail.rotationTitle", { tag: view.rotation.tag })) +
                  (canTurn ? ` ${t("detail.rotationClick")}` : "")
                }
              >
                <RotateCw className="h-3 w-3" /> {view.rotation.label}
              </button>
            )}
            {view.distance != null && (
              <span className="text-[11px] text-slate-400">
                {t("detail.distance", { value: decimal(view.distance, 3) })}
              </span>
            )}
            {view.via && <span className="text-[11px] text-slate-400">{view.via}</span>}
            {view.local && (
              <span className="text-[11px] text-slate-400">
                {view.local.model}
                {view.local.seconds != null && ` · ${decimal(view.local.seconds, 1)} s`}
              </span>
            )}
            {view.agent?.certainty && (
              <span className="text-[11px] text-slate-400">
                {t("detail.agentCertainty", { certainty: t(`status.confidence.${view.agent.certainty}` as Key) })}
              </span>
            )}
          </div>
          {view.agent?.note && (
            <p className="selectable mt-1 text-xs text-slate-500">{t("detail.agentNote", { note: view.agent.note })}</p>
          )}
        </div>
        {(onPrev || onNext) && (
          <div className="flex items-center">
            <button
              className="btn btn-ghost px-1.5"
              onClick={onPrev}
              disabled={!onPrev}
              title={t("detail.previousTitle")}
              aria-label={t("detail.previous")}
            >
              <ChevronUp className="h-4 w-4" />
            </button>
            <button
              className="btn btn-ghost px-1.5"
              onClick={onNext}
              disabled={!onNext}
              title={t("detail.nextTitle")}
              aria-label={t("detail.next")}
            >
              <ChevronDown className="h-4 w-4" />
            </button>
          </div>
        )}
        <button
          className="btn btn-ghost px-1.5"
          onClick={onClose}
          aria-label={t("detail.close")}
          title={t("detail.closeTitle")}
        >
          <X className="h-4 w-4" />
        </button>
      </div>
      {turning && canTurn && (
        <div className="flex items-center gap-2 border-b border-slate-200 bg-violet-50/70 px-4 py-2 text-xs text-violet-900 dark:border-slate-800 dark:bg-violet-950/30 dark:text-violet-200">
          <span className="flex-1">{t("detail.turn.text")}</span>
          <button className="btn px-2 py-0.5 text-xs" onClick={() => setTurning(false)}>
            {t("common.cancel")}
          </button>
          <button
            className="btn btn-primary px-2 py-0.5 text-xs"
            disabled={turnBusy}
            onClick={() => {
              setTurnBusy(true);
              onTurn!().finally(() => {
                setTurnBusy(false);
                setTurning(false);
              });
            }}
          >
            {turnBusy ? <Loader2 className="h-3 w-3 animate-spin" /> : <RotateCw className="h-3 w-3" />}{" "}
            {t("detail.turn.confirm")}
          </button>
        </div>
      )}
      {(view.toReview && onAccept) || view.byAgent || position ? (
        <div
          className={clsx(
            "flex items-center gap-2 border-b border-slate-200 px-4 py-2 text-xs dark:border-slate-800",
            view.toReview && onAccept && "bg-orange-50/70 dark:bg-orange-950/20",
            view.byAgent && "bg-indigo-50/70 dark:bg-indigo-950/20",
            view.accepted && !view.byAgent && "bg-emerald-50/60 dark:bg-emerald-950/20",
          )}
        >
          {view.byAgent ? (
            <>
              <span className="selectable flex flex-1 items-start gap-1.5 text-indigo-800 dark:text-indigo-300">
                <Bot className="h-4 w-4 shrink-0 text-indigo-500" strokeWidth={2.25} />
                <span>
                  {view.picked || view.edited ? t("detail.review.agentCorrected") : t("detail.review.agentAccepted")}
                  {view.reviewNote && (
                    <span className="text-indigo-700/80 dark:text-indigo-300/80">: {view.reviewNote}</span>
                  )}
                  {position && ` · ${position}`}
                </span>
              </span>
              {onAccept && (
                <button
                  className="btn btn-primary px-2 py-0.5 text-xs"
                  onClick={onAccept}
                  title={t("detail.review.confirmTitle")}
                >
                  <Check className="h-3 w-3" /> {t("detail.review.confirm")}
                </button>
              )}
              {onUnaccept && view.accepted && (
                <button
                  className="btn px-2 py-0.5 text-xs"
                  onClick={onUnaccept}
                  title={t("detail.review.unacceptTitle")}
                >
                  {t("detail.review.unaccept")}
                </button>
              )}
            </>
          ) : view.toReview && onAccept ? (
            <>
              <span className="flex flex-1 items-center gap-1.5 text-orange-800 dark:text-orange-300">
                <CircleQuestionMark className="h-4 w-4 shrink-0 text-orange-500" strokeWidth={2.5} />
                {view.confidence === "low" || view.confidence === "medium"
                  ? t("detail.review.unsure")
                  : t("detail.review.check")}
              </span>
              <button
                className="btn btn-primary px-2 py-0.5 text-xs"
                onClick={onAccept}
                title={t("detail.review.acceptTitle")}
              >
                <Check className="h-3 w-3" /> {t("detail.review.accept")}
              </button>
            </>
          ) : view.accepted ? (
            <>
              <span className="flex flex-1 items-center gap-1.5 text-emerald-800 dark:text-emerald-300">
                <CircleCheck className="h-4 w-4 shrink-0 text-emerald-600" strokeWidth={2.5} />
                {t("detail.review.accepted")}
                {position && ` · ${position}`}
              </span>
              {onUnaccept && (
                <button
                  className="btn px-2 py-0.5 text-xs"
                  onClick={onUnaccept}
                  title={t("detail.review.unacceptKeyTitle")}
                >
                  {t("detail.review.unaccept")}
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
                <RotateCcw className="h-3 w-3" /> {t("common.retry")}
              </button>
            )}
          </div>
        )}

        <Images images={view.images} active={side} onActive={setSide} onZoom={() => setZoomed(true)} />
        {zoomed && <Lightbox images={view.images} active={side} onActive={setSide} onClose={() => setZoomed(false)} />}

        {(view.price != null || view.grade != null || view.condition) && (
          <div className="grid grid-cols-2 gap-2">
            <Stat label={t("detail.marketPrice")} value={price(view.price, view.priceCurrency)} />
            <Stat
              label={
                view.gradingEndpoint === "centering"
                  ? t("detail.centering")
                  : view.gradingEndpoint === "condition"
                    ? t("detail.condition")
                    : t("detail.grade")
              }
              value={view.grade != null ? grade(view.grade) : (view.condition ?? "—")}
              sub={view.grade != null ? (view.condition ?? undefined) : undefined}
            />
            {view.priceNote && (
              <p className="col-span-2 text-xs text-slate-500">{t("detail.priceNote", { note: view.priceNote })}</p>
            )}
          </div>
        )}

        {extra}
        {ebay}

        {editing && view.edit && onEdit ? (
          <EditForm
            initial={view.edit}
            currency={view.priceCurrency}
            identified={view.status === "done" || view.status === "grading"}
            automaticTitle={title?.automatic}
            onCancel={() => setEditing(false)}
            onSave={(edit) => onEdit(edit).then(() => setEditing(false))}
          />
        ) : (
          (view.fields.length > 0 || view.edit?.notes || view.edit?.location || onEdit) && (
            <Section
              title={t("detail.card.title")}
              action={
                onEdit &&
                view.edit && (
                  <button
                    className="btn btn-ghost px-1.5 py-0.5 text-xs"
                    onClick={() => setEditing(true)}
                    title={t("detail.card.editTitle")}
                  >
                    <Pencil className="h-3 w-3" /> {t("detail.card.edit")}
                  </button>
                )
              }
            >
              <dl className="selectable grid grid-cols-[100px_1fr] gap-x-3 gap-y-1.5 text-sm">
                {view.fields.map(([k, v]) => (
                  <Field key={k} label={k} value={v} />
                ))}
                <Field label={t("detail.card.location")} value={view.edit?.location} />
                <Field label={t("detail.card.notes")} value={view.edit?.notes} />
              </dl>
              {title && (
                <div className="mt-3 border-t border-slate-100 pt-3 dark:border-slate-800">
                  <div className="mb-1 flex items-center gap-1.5 text-xs text-slate-500">
                    <span className="flex-1">{t("titles.listingTitle")}</span>
                    {title.custom && <CustomTitleMark />}
                    <TitleCount text={title.title} />
                  </div>
                  <div className="selectable text-sm font-medium break-words">{title.title}</div>
                  {!title.custom && title.leftOut.length > 0 && (
                    <div className="mt-0.5 text-xs text-slate-500">{leftOutText(title.leftOut)}</div>
                  )}
                </div>
              )}
            </Section>
          )
        )}

        {view.slab && Object.keys(view.slab).length > 0 && (
          <Section title={t("detail.slab.title")}>
            <dl className="selectable grid grid-cols-[100px_1fr] gap-x-3 gap-y-1.5 text-sm">
              <Field label={t("detail.slab.company")} value={view.slab.company} />
              <Field
                label={t("detail.slab.grade")}
                value={[view.slab.grade ?? view.slab.tagGrade, view.slab.verbal_grade].filter(Boolean).join(" · ")}
              />
              <Field label={t("detail.slab.certificate")} value={view.slab.certificate_number} />
              <Field label={t("detail.slab.label")} value={view.slab.name} />
            </dl>
          </Section>
        )}

        <Market market={view.market} onFind={onFindListings} onKeep={onKeepListings} defaultSite={ebaySite} />

        <Files images={view.images} />

        <Crops images={view.cropImages} />

        <Grading view={view} />

        {view.priceStats.length > 0 && (
          <Section title={t("detail.prices.title")}>
            <table className="w-full text-xs">
              <thead className="text-slate-500">
                <tr>
                  <th className="py-1 text-left font-medium">{t("detail.prices.type")}</th>
                  <th className="py-1 text-right font-medium">{t("detail.prices.median")}</th>
                  <th className="py-1 text-right font-medium">{t("detail.prices.min")}</th>
                  <th className="py-1 text-right font-medium">{t("detail.prices.max")}</th>
                  <th className="py-1 text-right font-medium">{t("detail.prices.latest")}</th>
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
          <Section title={t("detail.links")}>
            <div className="flex flex-wrap gap-1.5">
              {Object.entries(view.links).map(([name, url]) => (
                <button key={name} className="btn px-2 py-1 text-xs" onClick={() => openLink(url)}>
                  {name} <ExternalLink className="h-3 w-3" />
                </button>
              ))}
            </div>
          </Section>
        )}

        {view.alternatives.length > 0 && <Candidates list={view.alternatives} onPick={onPick} />}

        <details className="text-xs">
          <summary className="cursor-pointer font-medium text-slate-500">{t("detail.raw")}</summary>
          <pre className="selectable mt-2 max-h-96 overflow-auto rounded bg-slate-50 p-2 text-[11px] dark:bg-slate-900">
            {JSON.stringify(view.raw, null, 2)}
          </pre>
        </details>

        {onDelete && (
          <button
            className="btn btn-ghost w-full justify-center text-xs text-red-600 dark:text-red-400"
            onClick={onDelete}
          >
            <Trash2 className="h-3.5 w-3.5" /> {t("detail.deleteCard")}
          </button>
        )}
      </div>
    </aside>
  );
}

function Images({
  images,
  active,
  onActive,
  onZoom,
}: {
  images: CardView["images"];
  active: number;
  onActive: (i: number) => void;
  onZoom: () => void;
}) {
  const t = useT();
  const index = Math.min(active, images.length - 1);
  const img = images[index];
  if (!img) return null;
  return (
    <div>
      <div className="flex h-72 items-center justify-center rounded-lg bg-slate-100 p-2 dark:bg-slate-900">
        {img.src ? (
          <button
            className="flex h-full max-w-full cursor-zoom-in items-center justify-center"
            onClick={onZoom}
            title={t("detail.images.enlarge")}
          >
            <img src={img.src} alt={img.label} className="max-h-full max-w-full rounded object-contain shadow" />
          </button>
        ) : (
          <span className="text-sm text-slate-400">{t("detail.images.none")}</span>
        )}
      </div>
      <Sides images={images} active={index} onActive={onActive} className="mt-2" />
    </div>
  );
}

/** Front / back switches, when the card has both. */
function Sides({
  images,
  active,
  onActive,
  className,
  dark,
}: {
  images: CardView["images"];
  active: number;
  onActive: (i: number) => void;
  className?: string;
  dark?: boolean;
}) {
  if (images.length < 2) return null;
  return (
    <div className={clsx("flex justify-center gap-1", className)}>
      {images.map((im, i) => (
        <button
          key={im.label + i}
          onClick={() => onActive(i)}
          className={clsx(
            "rounded px-2 py-0.5 text-xs",
            i === active
              ? "bg-accent text-white"
              : dark
                ? "text-slate-300 hover:bg-white/10"
                : "btn-ghost text-slate-500",
          )}
        >
          {im.label}
        </button>
      ))}
    </div>
  );
}

/** The card's picture over the whole window: ← / → switch sides, O opens the original, Escape (or a click
 * beside it) closes. */
function Lightbox({
  images,
  active,
  onActive,
  onClose,
}: {
  images: CardView["images"];
  active: number;
  onActive: (i: number) => void;
  onClose: () => void;
}) {
  const t = useT();
  const index = Math.min(active, images.length - 1);
  const img = images[index];
  const backdrop = useBackdropClose(onClose);
  // Beside the picture is the backdrop too.
  const beside = useBackdropClose(onClose);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "Escape" || e.key === " ") onClose();
      else if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && images.length > 1)
        onActive((index + (e.key === "ArrowRight" ? 1 : images.length - 1)) % images.length);
      else if (e.key === "o" && img?.path) void openFile(img.path);
      else return;
      // The panel and the table behind it don't get the key.
      e.preventDefault();
      e.stopPropagation();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [images.length, index, img?.path, onActive, onClose]);
  if (!img) return null;
  return (
    <div
      className="fixed inset-0 z-50 flex flex-col bg-slate-950/90"
      role="dialog"
      aria-modal="true"
      aria-label={img.label}
      {...backdrop}
    >
      <div className="flex items-center gap-3 px-4 py-3">
        <Sides images={images} active={index} onActive={onActive} dark />
        <span className="flex-1" />
        {img.path && (
          <button
            className="flex items-center gap-1.5 rounded px-2 py-1 text-sm text-slate-200 hover:bg-white/10"
            onClick={() => void openFile(img.path!)}
          >
            <ExternalLink className="h-4 w-4" /> {t("detail.images.original")}
          </button>
        )}
        <button
          className="rounded p-1.5 text-slate-200 hover:bg-white/10"
          onClick={onClose}
          aria-label={t("common.close")}
        >
          <X className="h-5 w-5" />
        </button>
      </div>
      <div className="flex min-h-0 flex-1 items-center justify-center p-6 pt-0" {...beside}>
        {img.src && (
          <img src={img.src} alt={img.label} className="max-h-full max-w-full rounded object-contain shadow-2xl" />
        )}
      </div>
    </div>
  );
}

/** The scanned files the card was read from: name, folder, and a way to find them. */
function Files({ images }: { images: CardView["images"] }) {
  const t = useT();
  const files = images.filter((im) => im.path);
  if (!files.length) return null;
  return (
    <Section title={t("detail.files.title")}>
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
              onClick={() => reveal(path!)}
              title={t("detail.files.showInFolder")}
              aria-label={t("detail.files.showLabel", { label })}
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

function subgradeLabels(): Record<(typeof SUBGRADES)[number], string> {
  return {
    corners: t("detail.grading.corners"),
    edges: t("detail.grading.edges"),
    surface: t("detail.grading.surface"),
    centering: t("detail.grading.centering"),
  };
}

function Grading({ view }: { view: CardView }) {
  const t = useT();
  if (!view.gradingEndpoint) return null;
  const records = view.gradingRecords;
  const grades: Json = records.find((r: Json) => r?.grades)?.grades ?? {};
  const sides = records.filter((r: Json) => r?.grades);
  const centering = records[0]?.card?.[0]?.centering;
  const condition = records[0]?._objects?.[0]?.Condition?.[0];

  return (
    <Section
      title={t("detail.grading.title", {
        endpoint: view.gradingEndpoint === "imported" ? t("detail.grading.fromCsv") : view.gradingEndpoint,
      })}
    >
      {condition && (
        <p className="text-sm">
          <span className="font-semibold">{condition.label}</span>
          <span className="text-slate-500">
            {" "}
            {condition.value != null
              ? t("detail.grading.scaleValue", { mode: condition.mode, value: decimal(Number(condition.value), 2) })
              : t("detail.grading.scale", { mode: condition.mode })}
          </span>
        </p>
      )}
      {SUBGRADES.some((k) => grades[k] != null) && (
        <div className="space-y-1.5">
          {SUBGRADES.filter((k) => grades[k] != null).map((k) => (
            <div key={k} className="flex items-center gap-2 text-sm">
              <span className="w-20 text-slate-500">{subgradeLabels()[k]}</span>
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
          {sides
            .map(
              (r: Json, i: number) => `${i === 0 ? t("common.front") : t("common.back_side")} ${grade(r.grades.final)}`,
            )
            .join(" · ")}
        </p>
      )}
      {centering && (
        <p className="mt-2 text-xs text-slate-500">
          {t("detail.grading.centeringValues", { lr: centering["left/right"], tb: centering["top/bottom"] })}
        </p>
      )}
      {view.gradingImages.length > 0 && (
        <div className="mt-3 grid grid-cols-2 gap-2">
          {view.gradingImages.map((g) => (
            <button
              key={g.path ?? g.src}
              onClick={() => g.path && openFile(g.path)}
              className="overflow-hidden rounded border border-slate-200 dark:border-slate-800"
              title={t("detail.grading.openImage", { label: g.label })}
            >
              <img src={g.src} alt={g.label} className="h-32 w-full bg-slate-100 object-contain dark:bg-slate-900" />
            </button>
          ))}
        </div>
      )}
    </Section>
  );
}

/** The card's active eBay listings: the price range of those that are the card, and the listings. */
function Market({
  market,
  onFind,
  onKeep,
  defaultSite,
}: {
  market?: EbayMarket | null;
  onFind?: (marketplace: string) => Promise<unknown>;
  onKeep?: (ids: string[]) => Promise<unknown>;
  defaultSite?: string;
}) {
  const t = useT();
  // The site chosen here, else the one last searched, else Settings'.
  const [site, setSite] = useState<string | null>(null);
  const searchOn = site ?? market?.marketplace ?? defaultSite ?? "EBAY_US";
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!market && !onFind) return null;
  const run = (work: Promise<unknown>) => {
    setBusy(true);
    setError(null);
    work.catch((e) => setError(errorText(e))).finally(() => setBusy(false));
  };
  const find = () => onFind && run(onFind(searchOn));
  // The listing counts, or stops counting; the others stay as they are.
  const toggle = (id: string) =>
    market &&
    onKeep &&
    run(onKeep(market.listings.filter((l) => (l.id === id ? !l.counted : l.counted)).map((l) => l.id)));
  const range = market?.range;
  // The listings that are the card first, then those left out.
  const listings = market ? [...market.listings].sort((a, b) => Number(b.counted) - Number(a.counted)) : [];
  const leftOut = listings.filter((l) => !l.counted).length;
  return (
    <Section
      title={t("detail.market.title")}
      action={
        onFind && (
          <div className="flex items-center gap-1.5">
            <div className="w-36">
              <Select
                value={searchOn}
                onChange={setSite}
                options={ebaySiteOptions()}
                className="py-0.5 text-xs"
                aria-label={t("detail.market.site")}
              />
            </div>
            <button className="btn px-2 py-0.5 text-xs" onClick={find} disabled={busy}>
              {busy ? <Loader2 className="h-3 w-3 animate-spin" /> : <Search className="h-3 w-3" />}{" "}
              {market ? t("detail.market.again") : t("detail.market.search")}
            </button>
          </div>
        )
      }
    >
      <ErrorNote>{error}</ErrorNote>
      {!market ? (
        <p className="text-sm text-slate-500">{t("detail.market.none")}</p>
      ) : (
        <div className="space-y-2">
          {range ? (
            <div>
              <div className="text-lg font-semibold tabular-nums">
                {range.min === range.max
                  ? price(range.min, range.currency)
                  : `${price(range.min, range.currency)} – ${price(range.max, range.currency)}`}
              </div>
              <div className="text-xs text-slate-500">
                {t("detail.market.summary", {
                  count: range.count,
                  average: price(range.average, range.currency),
                  median: price(range.median, range.currency),
                })}
              </div>
            </div>
          ) : (
            <p className="text-sm text-slate-500">{t("detail.market.noPrices")}</p>
          )}
          {market.note && (
            <p className="flex items-start gap-1.5 text-xs text-slate-600 dark:text-slate-300">
              <Bot className="mt-0.5 h-3.5 w-3.5 shrink-0 text-violet-500" /> {market.note}
            </p>
          )}
          {listings.length > 0 && (
            <div className="max-h-72 overflow-y-auto rounded-lg border border-slate-200 dark:border-slate-800">
              <table className="w-full table-fixed text-xs">
                <tbody className="divide-y divide-slate-100 dark:divide-slate-800">
                  {listings.map((l) => (
                    <tr key={l.id} className={clsx(!l.counted && "opacity-45")}>
                      {onKeep && (
                        <td className="w-7 py-1.5 pl-2 align-top">
                          <input
                            type="checkbox"
                            className="h-3.5 w-3.5 accent-[var(--color-accent)]"
                            checked={l.counted}
                            disabled={busy}
                            onChange={() => toggle(l.id)}
                            title={t("detail.market.counts")}
                            aria-label={t("detail.market.counts")}
                          />
                        </td>
                      )}
                      <td className="px-2 py-1.5">
                        <button
                          className="block w-full truncate text-left hover:text-accent hover:underline"
                          onClick={() => l.url && openLink(l.url)}
                          title={l.title}
                        >
                          {l.title}
                        </button>
                        <span className="text-[10px] text-slate-400">
                          {[l.condition, l.buying.includes("AUCTION") ? t("detail.market.auction") : null, l.country]
                            .filter(Boolean)
                            .join(" · ")}
                          {!l.counted && ` · ${t("detail.market.leftOut")}`}
                        </span>
                      </td>
                      <td className="w-24 px-2 py-1.5 text-right align-top whitespace-nowrap tabular-nums">
                        {price(l.price, l.currency || "USD")}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <p className="text-[11px] text-slate-400">
            {t("detail.market.searched", {
              query: market.query,
              date: dateTime(market.searchedAt),
              total: market.total,
            })}
            {` · ${ebaySite(market.marketplace)}`}
            {leftOut > 0 && ` · ${t("detail.market.leftOutCount", { count: leftOut })}`}
          </p>
        </div>
      )}
    </Section>
  );
}

/** A crop job's pictures, large: they are what the session is for. */
function Crops({ images }: { images: CardView["cropImages"] }) {
  const t = useT();
  if (!images.length) return null;
  return (
    <Section title={t("detail.crop.title")}>
      <div className="grid grid-cols-2 gap-2">
        {images.map((g) => (
          <div key={g.path ?? g.src} className="space-y-1">
            <button
              onClick={() => g.path && openFile(g.path)}
              className="block w-full overflow-hidden rounded border border-slate-200 dark:border-slate-800"
              title={t("detail.grading.openImage", { label: g.label })}
            >
              <img src={g.src} alt={g.label} className="h-56 w-full bg-slate-100 object-contain dark:bg-slate-900" />
            </button>
            <button
              className="flex items-center gap-1 text-xs text-slate-500 hover:text-slate-800 dark:hover:text-slate-200"
              onClick={() => g.path && reveal(g.path)}
              title={t("detail.files.showLabel", { label: g.label })}
            >
              <FolderOpen className="h-3 w-3" /> {g.label}
            </button>
          </div>
        ))}
      </div>
    </Section>
  );
}

function Candidates({ list, onPick }: { list: Candidate[]; onPick?: (index: number) => Promise<unknown> }) {
  const t = useT();
  const [busy, setBusy] = useState<number | null>(null);
  const showPrice = list.some((c) => c.price != null);
  const pick = (index: number) => {
    if (!onPick) return;
    setBusy(index);
    onPick(index).finally(() => setBusy(null));
  };
  return (
    <Section title={t("detail.candidates.title", { count: list.length })}>
      <div className="divide-y divide-slate-100 overflow-hidden rounded-lg border border-slate-200 dark:divide-slate-800 dark:border-slate-800">
        {list.map((c, i) => (
          <div key={`${c.index}:${c.fullName}`} className="flex items-center gap-2.5 px-2.5 py-1.5 text-sm">
            {onPick && i < 9 && (
              <kbd
                className="w-4 shrink-0 text-center text-[11px] text-slate-400"
                title={t("detail.candidates.press", { n: i + 1 })}
              >
                {i + 1}
              </kbd>
            )}
            <div className="selectable min-w-0 flex-1" title={c.fullName || undefined}>
              <div className="truncate font-medium">{c.name}</div>
              {c.detail && <div className="truncate text-xs text-slate-500">{c.detail}</div>}
            </div>
            <div
              className="flex shrink-0 flex-col items-end gap-0.5"
              title={c.distance != null ? t("detail.distance", { value: decimal(c.distance, 3) }) : undefined}
            >
              {showPrice && <span className="text-xs tabular-nums">{price(c.price)}</span>}
              <ConfidenceChip value={c.confidence} />
            </div>
            {onPick && (
              <button
                className="btn px-2 py-0.5 text-xs"
                disabled={busy != null}
                onClick={() => pick(c.index)}
                title={t("detail.candidates.selectTitle")}
              >
                {busy === c.index ? <Loader2 className="h-3 w-3 animate-spin" /> : <Check className="h-3 w-3" />}
                {t("detail.candidates.select")}
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

function editFields(): [keyof ItemEdit, string][] {
  return [
    ["name", t("detail.edit.name")],
    ["setName", t("detail.edit.setName")],
    ["setCode", t("detail.edit.setCode")],
    ["cardNumber", t("detail.edit.cardNumber")],
    ["year", t("detail.edit.year")],
    ["series", t("status.field.series")],
    ["brand", t("status.field.brand")],
    ["team", t("status.field.team")],
    ["serial", t("status.field.serial")],
    ["features", t("status.field.features")],
  ];
}

/** The card's fields, price, location and notes as a form; Enter saves, Escape cancels. */
function EditForm({
  initial,
  currency,
  identified,
  automaticTitle,
  onSave,
  onCancel,
}: {
  initial: ItemEdit;
  /** The price's currency: an edit keeps it. */
  currency: string;
  /** Card fields and price can be typed only once the card is identified. */
  identified: boolean;
  /** The listing title the card's template makes: with it, the card can be given its own. */
  automaticTitle?: string;
  onSave: (edit: ItemEdit) => Promise<unknown>;
  onCancel: () => void;
}) {
  const t = useT();
  const [edit, setEdit] = useState(initial);
  const [priceText, setPriceText] = useState(initial.price != null ? String(initial.price) : "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (k: keyof ItemEdit, v: string) => setEdit((e) => ({ ...e, [k]: v }));

  const submit = () => {
    // Empty: no price.
    const empty = priceText.trim() === "";
    const value = empty ? null : parseAmount(priceText);
    if (!empty && (value == null || value < 0)) return setError(t("detail.edit.priceInvalid"));
    const length = titleLength(edit.title.trim());
    if (length > MAX_TITLE) return setError(t("titles.tooLong", { count: length, max: MAX_TITLE }));
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
    <Section title={t("detail.edit.title")}>
      <form
        className="grid grid-cols-[100px_1fr] gap-x-3 gap-y-1.5 text-sm"
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
        {editFields().map(([k, label], i) =>
          input(label, String(edit[k] ?? ""), (v) => set(k, v), !identified, identified && i === 0),
        )}
        {automaticTitle != null && (
          <label className="contents">
            <span className="pt-1.5 text-slate-500">{t("titles.listingTitle")}</span>
            <span>
              <textarea
                className="input min-h-14 py-1"
                value={edit.title}
                placeholder={automaticTitle}
                disabled={!identified}
                onChange={(e) => set("title", e.target.value.replace(/\n/g, " "))}
                onKeyDown={(e) => {
                  // Enter saves, as in the other fields: a title is one line.
                  if (e.key === "Enter") {
                    e.preventDefault();
                    submit();
                  }
                }}
              />
              <span className="mt-0.5 flex items-center gap-2 text-xs text-slate-500">
                <span className="flex-1">
                  {edit.title.trim() ? (
                    <button type="button" className="text-accent underline" onClick={() => set("title", "")}>
                      {t("titles.backToAutomatic")}
                    </button>
                  ) : (
                    t("titles.automaticHint")
                  )}
                </span>
                <TitleCount text={edit.title.trim() || automaticTitle} />
              </span>
            </span>
          </label>
        )}
        {input(t("detail.edit.price", { currency }), priceText, setPriceText, !identified)}
        {input(t("detail.edit.location"), edit.location, (v) => set("location", v), false, !identified)}
        <label className="contents">
          <span className="pt-1.5 text-slate-500">{t("detail.edit.notes")}</span>
          <textarea className="input min-h-14 py-1" value={edit.notes} onChange={(e) => set("notes", e.target.value)} />
        </label>
        <div className="col-span-2 mt-1 space-y-2">
          {!identified && <p className="text-xs text-slate-500">{t("detail.edit.notIdentified")}</p>}
          <ErrorNote>{error}</ErrorNote>
          <div className="flex justify-end gap-2">
            <button type="button" className="btn" onClick={onCancel}>
              {t("common.cancel")}
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy}>
              {t("common.save")}
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
