// Selling on eBay: the table badge, the detail panel section and the "List on eBay" dialog. Cards become
// unpublished offers (drafts) first; the session page publishes them.

import { openUrl } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import { ExternalLink, Send, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  api,
  errorText,
  type CardCondition,
  type EbayListing,
  type EbayPreview,
  type EbaySettings,
  type EbayStatus,
  type Session,
  type SettingsView,
} from "../lib/api";
import { t, useT } from "../i18n";
import { price } from "../lib/format";
import Modal, { ErrorNote } from "./Modal";
import { OpenSettings } from "./NewSessionDialog";
import PricedCards, { conditions, defaultNote, parsePrice } from "./PricedCards";
import Select from "./Select";

/** Listings know their policies and where cards ship from. */
export function ebayReady(e: EbaySettings): boolean {
  return !!(e.fulfillmentPolicyId && e.paymentPolicyId && e.returnPolicyId && e.locationKey);
}

export function ebayStatusLabels(): Record<EbayStatus, string> {
  return {
    draft: t("ebay.status.draft"),
    listed: t("ebay.status.listed"),
    sold: t("ebay.status.sold"),
    ended: t("ebay.status.ended"),
  };
}

const BADGE_STYLES: Record<EbayStatus | "error", string> = {
  draft: "bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300",
  listed: "bg-blue-100 text-blue-700 dark:bg-blue-950 dark:text-blue-300",
  sold: "bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300",
  ended: "bg-slate-200 text-slate-500 dark:bg-slate-800 dark:text-slate-400",
  error: "bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-300",
};

/** The card's eBay state in a few letters; the title says the rest. */
export function EbayBadge({ listing }: { listing?: EbayListing | null }) {
  const t = useT();
  if (!listing) return null;
  const key = listing.error && listing.status !== "sold" ? "error" : (listing.status ?? "error");
  const what = listing.status ? t(`ebay.badge.${listing.status}`) : "eBay";
  const title = [what, listing.price != null && price(listing.price), listing.error].filter(Boolean).join(" · ");
  return (
    <span className={clsx("rounded px-1 py-0.5 text-[10px] font-semibold", BADGE_STYLES[key])} title={title}>
      {listing.status === "sold" ? t("ebay.status.sold") : "eBay"}
    </span>
  );
}

/** The detail panel's eBay part: where the card is, the listing, and what can be done with it. */
export function EbaySection({
  listing,
  busy,
  onPublish,
  onRemove,
}: {
  listing: EbayListing;
  /** An eBay job runs for the session. */
  busy: boolean;
  onPublish: () => void;
  onRemove: () => void;
}) {
  const t = useT();
  const [url, setUrl] = useState<{ id: string; url: string } | null>(null);
  const id = listing.listingId;

  useEffect(() => {
    if (!id) return;
    let alive = true;
    api.ebayListingUrl(id).then((u) => alive && setUrl({ id, url: u }));
    return () => {
      alive = false;
    };
  }, [id]);

  const link = id && url?.id === id ? url.url : null;
  return (
    <section>
      <h3 className="mb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase">eBay</h3>
      <div className="space-y-2 rounded-lg bg-slate-50 px-3 py-2 text-sm dark:bg-slate-900">
        <div className="flex items-center gap-2">
          <span className="font-medium">
            {listing.status ? ebayStatusLabels()[listing.status] : t("ebay.section.notOnEbay")}
          </span>
          {listing.price != null && <span className="text-slate-500 tabular-nums">{price(listing.price)}</span>}
          <span className="flex-1" />
          {link && (
            <button
              className="flex items-center gap-1 text-xs text-accent hover:underline"
              onClick={() => openUrl(link)}
            >
              {t("ebay.section.listing", { id: id ?? "" })} <ExternalLink className="h-3 w-3" />
            </button>
          )}
        </div>
        {listing.status === "draft" && <p className="text-xs text-slate-500">{t("ebay.section.draftNote")}</p>}
        {listing.error && <p className="selectable text-xs text-red-600 dark:text-red-400">{listing.error}</p>}
        {listing.status !== "sold" && (
          <div className="flex gap-2">
            {listing.status === "draft" && (
              <button className="btn px-2 py-0.5 text-xs" disabled={busy} onClick={onPublish}>
                <Send className="h-3 w-3" /> {t("ebay.section.publish")}
              </button>
            )}
            {(listing.status === "draft" || listing.status === "listed" || listing.status === "ended") && (
              <button className="btn px-2 py-0.5 text-xs" disabled={busy} onClick={onRemove}>
                <Trash2 className="h-3 w-3" />{" "}
                {listing.status === "listed" ? t("ebay.section.endListing") : t("ebay.section.remove")}
              </button>
            )}
          </div>
        )}
      </div>
    </section>
  );
}

/** Turns a session's identified cards into eBay drafts at the prices shown. */
export function EbayListDialog({ session, onClose }: { session: Session; onClose: () => void }) {
  const t = useT();
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [cards, setCards] = useState<EbayPreview[] | null>(null);
  const [chosen, setChosen] = useState<Set<number>>(new Set());
  const [prices, setPrices] = useState<Record<number, string>>({});
  const [condition, setCondition] = useState<CardCondition>("near_mint");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getSettings()
      .then(setSettings)
      .catch((e) => setError(errorText(e)));
    api
      .ebayPreview(session.id)
      .then((list) => {
        setCards(list);
        // Cards already on eBay are chosen only to change them.
        setChosen(new Set(list.filter((c) => listable(c) && !c.ebay?.status).map((c) => c.itemId)));
        setPrices(Object.fromEntries(list.map((c) => [c.itemId, c.price != null ? c.price.toFixed(2) : ""])));
      })
      .catch((e) => setError(errorText(e)));
  }, [session.id]);

  const picked = useMemo(() => (cards ?? []).filter((c) => chosen.has(c.itemId)), [cards, chosen]);
  const unpriced = picked.filter((c) => parsePrice(prices[c.itemId]) == null).length;
  const total = picked.reduce((sum, c) => sum + (parsePrice(prices[c.itemId]) ?? 0), 0);
  const ungraded = picked.some((c) => c.condition === "Ungraded");
  const missing = !settings
    ? null
    : !settings.ebayConnected
      ? t("ebay.list.connectFirst")
      : !ebayReady(settings.ebay)
        ? t("ebay.list.policiesFirst")
        : null;
  const submit = () => {
    setBusy(true);
    setError(null);
    const items = picked.map((c) => ({ itemId: c.itemId, price: parsePrice(prices[c.itemId])! }));
    api
      .ebayCreateDrafts(session.id, items, condition)
      .then(onClose)
      .catch((e) => {
        setError(errorText(e));
        setBusy(false);
      });
  };

  return (
    <Modal
      title={t("ebay.list.title")}
      width="max-w-3xl"
      onClose={onClose}
      footer={
        <>
          <span className="mr-auto self-center text-sm text-slate-500">
            {t("ebay.list.total", { cards: t("common.cards", { count: picked.length }), total: price(total) })}
            {unpriced > 0 && (
              <span className="text-red-600"> · {t("ebay.list.withoutPrice", { count: unpriced })}</span>
            )}
          </span>
          <button className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button
            className="btn btn-primary"
            disabled={busy || !!missing || !settings || picked.length === 0 || unpriced > 0}
            onClick={submit}
          >
            {busy ? t("ebay.list.starting") : t("ebay.list.createDrafts", { count: picked.length })}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        {missing && (
          <ErrorNote>
            <div className="flex items-center gap-3">
              <span className="flex-1">{missing}</span>
              <OpenSettings section="ebay" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        <p className="text-sm text-slate-500">
          {settings?.ebay.sandbox ? t("ebay.list.introSandbox") : t("ebay.list.intro")}
        </p>
        <div className="flex items-end gap-4">
          <div className="w-72">
            <div className="label">{t("ebay.list.condition")}</div>
            <Select
              value={condition}
              options={conditions()}
              onChange={setCondition}
              disabled={!ungraded}
              aria-label={t("ebay.list.condition")}
            />
          </div>
          <p className="pb-2 text-xs text-slate-500">{t("ebay.list.gradedNote")}</p>
        </div>
        <ErrorNote>{error}</ErrorNote>
        {!cards ? (
          <p className="text-sm text-slate-500">{t("ebay.list.loadingCards")}</p>
        ) : cards.length === 0 ? (
          <p className="text-sm text-slate-500">{t("ebay.list.noCards")}</p>
        ) : (
          <PricedCards
            cards={cards}
            chosen={chosen}
            onChosen={setChosen}
            prices={prices}
            onPrices={setPrices}
            can={listable}
            note={ebayNote}
            badge={(c) => <EbayBadge listing={c.ebay} />}
            verb="listed"
          />
        )}
      </div>
    </Modal>
  );
}

function listable(c: EbayPreview): boolean {
  return !c.problem && c.ebay?.status !== "sold";
}

function ebayNote(c: EbayPreview): ReactNode {
  const note = defaultNote(c);
  if (c.problem) return note;
  // Without a problem the note is plain text.
  if (c.ebay?.status === "sold") return t("ebay.note.sold");
  if (c.ebay?.status === "listed") return t("ebay.note.live", { note: String(note) });
  if (c.ebay?.status === "draft") return t("ebay.note.draft", { note: String(note) });
  return note;
}
