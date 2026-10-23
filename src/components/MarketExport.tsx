// Exporting a session's cards for TCGplayer (a list of ungraded cards to add in Seller Portal) or Whatnot (its
// bulk listing CSV, with pictures uploaded to ImgBB).

import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { FolderOpen } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
  api,
  errorText,
  onExportProgress,
  type CardCondition,
  type EbayPreview,
  type ExportFormat,
  type Session,
  type SettingsView,
  type ShopifyOptions,
  type Skipped,
  type WhatnotOptions,
} from "../lib/api";
import { price } from "../lib/format";
import Modal, { ErrorNote } from "./Modal";
import { Check, OpenSettings } from "./NewSessionDialog";
import PricedCards, { CONDITIONS, parsePrice } from "./PricedCards";
import Select, { type SelectOption } from "./Select";

const NAMES: Record<ExportFormat, string> = { tcgplayer: "TCGplayer", whatnot: "Whatnot", shopify: "Shopify" };

/** Whatnot's US shipping profiles; the seller's own profile names work too. */
const SHIPPING_PROFILES = [
  "0-1 oz",
  "1-3 oz",
  "4-7 oz",
  "8-11 oz",
  "12-15 oz",
  "1 lb",
  "1-2 lbs",
  "Sports singles (3oz)",
];

const LISTING_TYPES: SelectOption<WhatnotOptions["listingType"]>[] = [
  { value: "auction", label: "Auction", hint: "the price is the starting bid" },
  { value: "buy_it_now", label: "Buy it Now", hint: "a fixed price" },
];

const SHOPIFY_FILES: SelectOption<ShopifyOptions["file"]>[] = [
  { value: "products", label: "Products", hint: "new products with variants" },
  { value: "inventory", label: "Inventory update", hint: "existing products" },
];

const SHOPIFY_GROUPING: SelectOption<ShopifyOptions["grouping"]>[] = [
  { value: "by_card", label: "One product per card", hint: "conditions are variants" },
  { value: "per_card", label: "One product per scanned card" },
];

interface Done {
  path: string;
  /** "12 cards", "8 products (11 variants)". */
  what: string;
  skipped: Skipped[];
  withoutImages?: number;
  imageError?: string | null;
}

export function MarketExportDialog({
  session,
  format,
  onClose,
}: {
  session: Session;
  format: ExportFormat;
  onClose: () => void;
}) {
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [cards, setCards] = useState<EbayPreview[] | null>(null);
  const [chosen, setChosen] = useState<Set<number>>(new Set());
  const [prices, setPrices] = useState<Record<number, string>>({});
  const [condition, setCondition] = useState<CardCondition>("near_mint");
  const [whatnot, setWhatnot] = useState<Omit<WhatnotOptions, "condition" | "uploadImages">>({
    listingType: "auction",
    shippingProfile: "0-1 oz",
    offerable: false,
  });
  const [shopify, setShopify] = useState<Omit<ShopifyOptions, "condition" | "uploadImages">>({
    file: "products",
    grouping: "by_card",
    active: false,
    location: "",
  });
  const [upload, setUpload] = useState(true);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [done, setDone] = useState<Done | null>(null);
  const [error, setError] = useState<string | null>(null);
  const name = NAMES[format];
  const inventory = format === "shopify" && shopify.file === "inventory";
  const pictures = format === "whatnot" || (format === "shopify" && !inventory);

  useEffect(() => {
    api
      .getSettings()
      .then(setSettings)
      .catch((e) => setError(errorText(e)));
    api
      .exportPreview(session.id, format)
      .then((list) => {
        setCards(list);
        setChosen(new Set(list.filter((c) => !c.problem).map((c) => c.itemId)));
        setPrices(
          Object.fromEntries(
            list.map((c) => [
              c.itemId,
              c.price == null ? "" : format === "whatnot" ? String(c.price) : c.price.toFixed(2),
            ]),
          ),
        );
      })
      .catch((e) => setError(errorText(e)));
    const un = onExportProgress((p) => {
      if (p.sessionId === session.id) setProgress({ done: p.done, total: p.total });
    });
    return () => {
      void un.then((f) => f());
    };
  }, [session.id, format]);

  const picked = useMemo(() => (cards ?? []).filter((c) => chosen.has(c.itemId)), [cards, chosen]);
  const unpriced = inventory ? 0 : picked.filter((c) => parsePrice(prices[c.itemId]) == null).length;
  const total = picked.reduce((sum, c) => sum + (parsePrice(prices[c.itemId]) ?? 0), 0);
  const keySet = !!settings?.imgbbKeySet;

  const run = async () => {
    setError(null);
    const path = await save({
      defaultPath: `${session.name.replace(/[\\/:*?"<>|]+/g, "-")} - ${name}${inventory ? " inventory" : ""}.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    setBusy(true);
    setProgress(null);
    const items = picked.map((c) => ({ itemId: c.itemId, price: parsePrice(prices[c.itemId]) ?? 0 }));
    const cards = (n: number) => `${n} card${n === 1 ? "" : "s"}`;
    try {
      if (format === "tcgplayer") {
        const r = await api.exportTcgplayer(session.id, items, condition, path);
        setDone({ path, what: cards(r.rows), ...r });
      } else if (format === "whatnot") {
        const r = await api.exportWhatnot(
          session.id,
          items,
          { ...whatnot, condition, uploadImages: upload && keySet },
          path,
        );
        setDone({ path, what: cards(r.rows), ...r });
      } else {
        const r = await api.exportShopify(
          session.id,
          items,
          { ...shopify, condition, uploadImages: upload && keySet },
          path,
        );
        const products = `${r.products} product${r.products === 1 ? "" : "s"}`;
        setDone({ path, what: `${products} (${r.variants} variant${r.variants === 1 ? "" : "s"})`, ...r });
      }
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  if (done) return <Result name={name} done={done} onClose={onClose} />;

  return (
    <Modal
      title={`Export for ${name}`}
      width="max-w-3xl"
      onClose={onClose}
      footer={
        <>
          <span className="mr-auto self-center text-sm text-slate-500">
            {busy && progress
              ? `${pictures && keySet && upload ? "Uploading pictures…" : "Preparing…"} ${progress.done}/${progress.total}`
              : `${picked.length} card${picked.length === 1 ? "" : "s"}${inventory ? "" : ` · ${price(total)}`}`}
            {!busy && unpriced > 0 && <span className="text-red-600"> · {unpriced} without a price</span>}
          </span>
          {busy && format !== "tcgplayer" ? (
            <button className="btn" onClick={() => api.cancelExport(session.id)}>
              Stop
            </button>
          ) : (
            <button className="btn" onClick={onClose}>
              Cancel
            </button>
          )}
          <button
            className="btn btn-primary"
            disabled={busy || !cards || picked.length === 0 || unpriced > 0 || (inventory && !shopify.location.trim())}
            onClick={run}
          >
            {busy ? "Exporting…" : "Export CSV…"}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        {format === "tcgplayer" ? (
          <p className="text-sm text-slate-500">
            A list of the ungraded trading cards in TCGplayer’s terms — product line, set, name, number, condition and
            your price — with each card’s TCGplayer product link. TCGplayer imports only files from its own Seller
            Portal export, so add the cards in Seller Portal from this list. Graded cards are sold there as Listings
            with Photos, one by one, and TCGplayer has no sports cards.
          </p>
        ) : format === "shopify" ? (
          <p className="text-sm text-slate-500">
            {inventory
              ? "Shopify’s inventory file (Products → Inventory → Import): sets each product’s stock at the location to the number of copies here. It finds the products this session’s product export made — same grouping — by their handles and conditions."
              : "Shopify’s product file (Products → Import). Cards are found again by their handles, so tick “Overwrite products with matching handles” to update products imported before. Graded slabs are products of their own. In stores with several locations, set stock with the inventory update afterwards."}
          </p>
        ) : (
          <p className="text-sm text-slate-500">
            Whatnot’s bulk listing file: import it in Seller Hub (Inventory → Import from CSV) for draft listings, or in
            a show (Add → Create Temporary Listing → Upload CSV). Graded slabs are listed as Graded with the grader and
            grade in the title and description. Prices are whole dollars, rounded up.
          </p>
        )}
        <div className="grid grid-cols-2 gap-4">
          {format === "whatnot" && (
            <>
              <div>
                <div className="label">Type</div>
                <Select
                  value={whatnot.listingType}
                  options={LISTING_TYPES}
                  onChange={(listingType) => setWhatnot({ ...whatnot, listingType })}
                  aria-label="Listing type"
                />
              </div>
              <div>
                <div className="label">Shipping profile</div>
                <Select
                  value={whatnot.shippingProfile}
                  options={SHIPPING_PROFILES.map((p) => ({ value: p, label: p }))}
                  onChange={(shippingProfile) => setWhatnot({ ...whatnot, shippingProfile })}
                  aria-label="Shipping profile"
                />
              </div>
            </>
          )}
          {format === "shopify" && (
            <>
              <div>
                <div className="label">File</div>
                <Select
                  value={shopify.file}
                  options={SHOPIFY_FILES}
                  onChange={(file) => setShopify({ ...shopify, file })}
                  aria-label="Shopify file"
                />
              </div>
              <div>
                <div className="label">Products</div>
                <Select
                  value={shopify.grouping}
                  options={SHOPIFY_GROUPING}
                  onChange={(grouping) => setShopify({ ...shopify, grouping })}
                  aria-label="Products"
                />
              </div>
            </>
          )}
          <div>
            <div className="label">Condition of ungraded cards</div>
            <Select
              value={condition}
              options={CONDITIONS}
              onChange={setCondition}
              aria-label="Condition of ungraded cards"
            />
            <p className="mt-1 text-xs text-slate-500">Cards whose condition Ximilar graded keep theirs.</p>
          </div>
          {inventory && (
            <div>
              <label className="label">Location</label>
              <input
                className="input"
                value={shopify.location}
                placeholder="e.g. Shop location"
                onChange={(e) => setShopify({ ...shopify, location: e.target.value })}
              />
              <p className="mt-1 text-xs text-slate-500">Exactly as named in Shopify (Settings → Locations).</p>
            </div>
          )}
          {format === "shopify" && !inventory && (
            <div className="space-y-2 pt-5">
              <Check
                label="Upload the pictures (ImgBB)"
                checked={upload && keySet}
                disabled={!keySet}
                onChange={setUpload}
              />
              <Check
                label="Publish (active) instead of drafts"
                checked={shopify.active}
                onChange={(active) => setShopify({ ...shopify, active })}
              />
            </div>
          )}
          {format === "whatnot" && (
            <div className="space-y-2 pt-5">
              <Check
                label="Upload the pictures (ImgBB)"
                checked={upload && keySet}
                disabled={!keySet}
                onChange={setUpload}
              />
              {whatnot.listingType === "buy_it_now" && (
                <Check
                  label="Buyers can make offers"
                  checked={whatnot.offerable}
                  onChange={(offerable) => setWhatnot({ ...whatnot, offerable })}
                />
              )}
            </div>
          )}
        </div>
        {pictures && settings && !keySet && (
          <ErrorNote>
            <div className="flex items-center gap-3">
              <span className="flex-1">
                {name} downloads the pictures from links. Add an ImgBB API key in Settings to upload them; without one,
                add the photos in {name}.
              </span>
              <OpenSettings section="ebay" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        <ErrorNote>{error}</ErrorNote>
        {!cards ? (
          <p className="text-sm text-slate-500">Loading cards…</p>
        ) : cards.length === 0 ? (
          <p className="text-sm text-slate-500">No identified cards in this session yet.</p>
        ) : (
          <PricedCards
            cards={cards}
            chosen={chosen}
            onChosen={setChosen}
            prices={prices}
            onPrices={setPrices}
            can={(c) => !c.problem}
            verb="exported"
            showPrices={!inventory}
          />
        )}
        {format === "shopify" && shopify.grouping === "by_card" && !inventory && (
          <p className="text-xs text-slate-500">
            Copies of a card in the same condition become one variant, at the highest of their prices.
          </p>
        )}
      </div>
    </Modal>
  );
}

function Result({ name, done, onClose }: { name: string; done: Done; onClose: () => void }) {
  return (
    <Modal
      title={`Exported for ${name}`}
      width="max-w-lg"
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={() => revealItemInDir(done.path)}>
            <FolderOpen className="h-4 w-4" /> Show file
          </button>
          <button className="btn btn-primary" onClick={onClose}>
            Done
          </button>
        </>
      }
    >
      <div className="selectable space-y-3 text-sm text-slate-600 dark:text-slate-300">
        <p>
          {done.what} saved to <span className="break-all">{done.path}</span>.
        </p>
        {!!done.withoutImages && (
          <p className="text-amber-700 dark:text-amber-400">
            {done.withoutImages} without pictures: add them in {name} before publishing.
          </p>
        )}
        <ErrorNote>{done.imageError}</ErrorNote>
        {done.skipped.length > 0 && (
          <div>
            <p className="mb-1">Left out:</p>
            <ul className="max-h-40 list-inside list-disc overflow-y-auto text-xs">
              {done.skipped.map((s) => (
                <li key={s.seq}>
                  #{s.seq}: {s.reason}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </Modal>
  );
}
