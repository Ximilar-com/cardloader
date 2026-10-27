// Exporting a session's cards for TCGplayer (a list of ungraded cards to add in Seller Portal) or Whatnot (its
// bulk listing CSV, with pictures uploaded to ImgBB).

import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { FolderOpen } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { rich, t as translate, useT } from "../i18n";
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
import PricedCards, { conditions, parsePrice } from "./PricedCards";
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

function listingTypes(): SelectOption<WhatnotOptions["listingType"]>[] {
  return [
    {
      value: "auction",
      label: translate("market.listingType.auction"),
      hint: translate("market.listingType.auctionHint"),
    },
    {
      value: "buy_it_now",
      label: translate("market.listingType.buyItNow"),
      hint: translate("market.listingType.buyItNowHint"),
    },
  ];
}

function shopifyFiles(): SelectOption<ShopifyOptions["file"]>[] {
  return [
    {
      value: "products",
      label: translate("market.shopifyFile.products"),
      hint: translate("market.shopifyFile.productsHint"),
    },
    {
      value: "inventory",
      label: translate("market.shopifyFile.inventory"),
      hint: translate("market.shopifyFile.inventoryHint"),
    },
  ];
}

function shopifyGrouping(): SelectOption<ShopifyOptions["grouping"]>[] {
  return [
    {
      value: "by_card",
      label: translate("market.shopifyGrouping.byCard"),
      hint: translate("market.shopifyGrouping.byCardHint"),
    },
    { value: "per_card", label: translate("market.shopifyGrouping.perCard") },
  ];
}

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
  const t = useT();
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
    const cards = (n: number) => t("common.cards", { count: n });
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
        const what = t("market.result.productsVariants", {
          products: t("market.result.products", { count: r.products }),
          variants: t("market.result.variants", { count: r.variants }),
        });
        setDone({ path, what, ...r });
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
      title={t("market.export.title", { name })}
      width="max-w-3xl"
      onClose={onClose}
      footer={
        <>
          <span className="mr-auto self-center text-sm text-slate-500">
            {busy && progress
              ? t(
                  pictures && keySet && upload ? "market.export.uploadingProgress" : "market.export.preparingProgress",
                  { done: progress.done, total: progress.total },
                )
              : inventory
                ? t("common.cards", { count: picked.length })
                : t("market.export.total", { cards: t("common.cards", { count: picked.length }), total: price(total) })}
            {!busy && unpriced > 0 && (
              <span className="text-red-600"> · {t("market.export.withoutPrice", { count: unpriced })}</span>
            )}
          </span>
          {busy && format !== "tcgplayer" ? (
            <button className="btn" onClick={() => api.cancelExport(session.id)}>
              {t("market.export.stop")}
            </button>
          ) : (
            <button className="btn" onClick={onClose}>
              {t("common.cancel")}
            </button>
          )}
          <button
            className="btn btn-primary"
            disabled={busy || !cards || picked.length === 0 || unpriced > 0 || (inventory && !shopify.location.trim())}
            onClick={run}
          >
            {busy ? t("market.export.exporting") : t("market.export.exportCsv")}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        {format === "tcgplayer" ? (
          <p className="text-sm text-slate-500">{t("market.export.tcgplayerIntro")}</p>
        ) : format === "shopify" ? (
          <p className="text-sm text-slate-500">
            {inventory ? t("market.export.shopifyInventoryIntro") : t("market.export.shopifyProductsIntro")}
          </p>
        ) : (
          <p className="text-sm text-slate-500">{t("market.export.whatnotIntro")}</p>
        )}
        <div className="grid grid-cols-2 gap-4">
          {format === "whatnot" && (
            <>
              <div>
                <div className="label">{t("market.export.type")}</div>
                <Select
                  value={whatnot.listingType}
                  options={listingTypes()}
                  onChange={(listingType) => setWhatnot({ ...whatnot, listingType })}
                  aria-label={t("market.export.listingType")}
                />
              </div>
              <div>
                <div className="label">{t("market.export.shippingProfile")}</div>
                <Select
                  value={whatnot.shippingProfile}
                  options={SHIPPING_PROFILES.map((p) => ({ value: p, label: p }))}
                  onChange={(shippingProfile) => setWhatnot({ ...whatnot, shippingProfile })}
                  aria-label={t("market.export.shippingProfile")}
                />
              </div>
            </>
          )}
          {format === "shopify" && (
            <>
              <div>
                <div className="label">{t("market.export.file")}</div>
                <Select
                  value={shopify.file}
                  options={shopifyFiles()}
                  onChange={(file) => setShopify({ ...shopify, file })}
                  aria-label={t("market.export.shopifyFile")}
                />
              </div>
              <div>
                <div className="label">{t("market.export.products")}</div>
                <Select
                  value={shopify.grouping}
                  options={shopifyGrouping()}
                  onChange={(grouping) => setShopify({ ...shopify, grouping })}
                  aria-label={t("market.export.products")}
                />
              </div>
            </>
          )}
          <div>
            <div className="label">{t("market.export.condition")}</div>
            <Select
              value={condition}
              options={conditions()}
              onChange={setCondition}
              aria-label={t("market.export.condition")}
            />
            <p className="mt-1 text-xs text-slate-500">{t("market.export.conditionNote")}</p>
          </div>
          {inventory && (
            <div>
              <label className="label">{t("market.export.location")}</label>
              <input
                className="input"
                value={shopify.location}
                placeholder={t("market.export.locationPlaceholder")}
                onChange={(e) => setShopify({ ...shopify, location: e.target.value })}
              />
              <p className="mt-1 text-xs text-slate-500">{t("market.export.locationNote")}</p>
            </div>
          )}
          {format === "shopify" && !inventory && (
            <div className="space-y-2 pt-5">
              <Check
                label={t("market.export.uploadPictures")}
                checked={upload && keySet}
                disabled={!keySet}
                onChange={setUpload}
              />
              <Check
                label={t("market.export.publishActive")}
                checked={shopify.active}
                onChange={(active) => setShopify({ ...shopify, active })}
              />
            </div>
          )}
          {format === "whatnot" && (
            <div className="space-y-2 pt-5">
              <Check
                label={t("market.export.uploadPictures")}
                checked={upload && keySet}
                disabled={!keySet}
                onChange={setUpload}
              />
              {whatnot.listingType === "buy_it_now" && (
                <Check
                  label={t("market.export.offerable")}
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
              <span className="flex-1">{t("market.export.noImgbbKey", { name })}</span>
              <OpenSettings section="ebay" onOpen={onClose} />
            </div>
          </ErrorNote>
        )}
        <ErrorNote>{error}</ErrorNote>
        {!cards ? (
          <p className="text-sm text-slate-500">{t("market.export.loadingCards")}</p>
        ) : cards.length === 0 ? (
          <p className="text-sm text-slate-500">{t("market.export.noCards")}</p>
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
          <p className="text-xs text-slate-500">{t("market.export.variantNote")}</p>
        )}
      </div>
    </Modal>
  );
}

function Result({ name, done, onClose }: { name: string; done: Done; onClose: () => void }) {
  const t = useT();
  return (
    <Modal
      title={t("market.result.title", { name })}
      width="max-w-lg"
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={() => revealItemInDir(done.path)}>
            <FolderOpen className="h-4 w-4" /> {t("market.result.showFile")}
          </button>
          <button className="btn btn-primary" onClick={onClose}>
            {t("market.result.done")}
          </button>
        </>
      }
    >
      <div className="selectable space-y-3 text-sm text-slate-600 dark:text-slate-300">
        <p>
          {rich(t("market.result.saved", { what: done.what }), {
            path: () => <span className="break-all">{done.path}</span>,
          })}
        </p>
        {!!done.withoutImages && (
          <p className="text-amber-700 dark:text-amber-400">
            {t("market.result.withoutPictures", { count: done.withoutImages, name })}
          </p>
        )}
        <ErrorNote>{done.imageError}</ErrorNote>
        {done.skipped.length > 0 && (
          <div>
            <p className="mb-1">{t("market.result.leftOut")}</p>
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
