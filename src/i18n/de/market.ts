import type { Messages } from "..";
import type en from "../en/market";

const messages: Messages<typeof en> = {
  conditionHint: {
    excellent: "leicht bespielt",
    veryGood: "mäßig bespielt",
    poor: "stark bespielt",
  },
  priced: {
    allCards: "Alle Karten",
    canBeListed: {
      one: "{count} Karte kann angeboten werden",
      other: "{count} Karten können angeboten werden",
    },
    canBeExported: {
      one: "{count} Karte kann exportiert werden",
      other: "{count} Karten können exportiert werden",
    },
    adjustPrices: "Preise anpassen",
    priceUsd: "Preis (USD)",
    percentOfMarket: "<field></field> % des Marktpreises",
    percentLabel: "Prozent des Marktpreises",
    endIn99: "auf ,99 enden",
    atLeast: "mindestens $<field></field>",
    lowestPrice: "Niedrigster Preis",
    apply: "Auf die gewählten Karten anwenden",
    choose: "#{seq} wählen",
    price: "Preis",
    priceOf: "Preis von #{seq}",
  },
  listingType: {
    auction: "Auktion",
    auctionHint: "der Preis ist das Startgebot",
    buyItNow: "Sofort-Kaufen",
    buyItNowHint: "ein Festpreis",
  },
  shopifyFile: {
    products: "Produkte",
    productsHint: "neue Produkte mit Varianten",
    inventory: "Bestand aktualisieren",
    inventoryHint: "bestehende Produkte",
  },
  shopifyGrouping: {
    byCard: "Ein Produkt pro Karte",
    byCardHint: "Zustände sind Varianten",
    perCard: "Ein Produkt pro gescannter Karte",
  },
  export: {
    title: "Export für {name}",
    uploadingProgress: "Bilder werden hochgeladen… {done}/{total}",
    preparingProgress: "Wird vorbereitet… {done}/{total}",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} ohne Preis", other: "{count} ohne Preis" },
    stop: "Stoppen",
    exporting: "Wird exportiert…",
    exportCsv: "CSV exportieren…",
    tcgplayerIntro:
      "Eine Liste der unbewerteten Sammelkarten in TCGplayers Begriffen — Produktlinie, Set, Name, Nummer, Zustand und dein Preis — mit dem TCGplayer-Produktlink jeder Karte. TCGplayer importiert nur Dateien aus seinem eigenen Seller-Portal-Export, also füge die Karten anhand dieser Liste im Seller Portal hinzu. Bewertete Karten werden dort einzeln als Listings with Photos verkauft, und Sportkarten gibt es bei TCGplayer nicht.",
    shopifyInventoryIntro:
      "Shopifys Bestandsdatei (Produkte → Inventar → Importieren): setzt den Bestand jedes Produkts am Standort auf die Anzahl der Exemplare hier. Sie findet die Produkte aus dem Produktexport dieser Sitzung — gleiche Gruppierung — über ihre Handles und Zustände.",
    shopifyProductsIntro:
      "Shopifys Produktdatei (Produkte → Importieren). Karten werden über ihre Handles wiedergefunden, also hake „Produkte mit übereinstimmenden Handles überschreiben“ an, um früher importierte Produkte zu aktualisieren. Bewertete Slabs sind eigene Produkte. In Shops mit mehreren Standorten setzt du den Bestand danach mit der Bestandsaktualisierung.",
    whatnotIntro:
      "Whatnots Datei für Sammelangebote: Importiere sie im Seller Hub (Inventory → Import from CSV) für Angebotsentwürfe oder in einer Show (Add → Create Temporary Listing → Upload CSV). Bewertete Slabs werden als Graded angeboten, mit Grader und Bewertung in Titel und Beschreibung. Preise sind ganze Dollar, aufgerundet.",
    type: "Typ",
    listingType: "Angebotsart",
    shippingProfile: "Versandprofil",
    file: "Datei",
    shopifyFile: "Shopify-Datei",
    products: "Produkte",
    condition: "Zustand unbewerteter Karten",
    conditionNote: "Karten, deren Zustand Ximilar bewertet hat, behalten ihn.",
    location: "Standort",
    locationPlaceholder: "z. B. Ladengeschäft",
    locationNote: "Genau wie in Shopify benannt (Einstellungen → Standorte).",
    uploadPictures: "Bilder hochladen (ImgBB)",
    publishActive: "Veröffentlichen (aktiv) statt Entwürfe",
    offerable: "Käufer können Preisvorschläge machen",
    noImgbbKey:
      "{name} lädt die Bilder über Links herunter. Füge in den Einstellungen einen ImgBB-API-Schlüssel hinzu, um sie hochzuladen; ohne ihn füge die Fotos in {name} hinzu.",
    loadingCards: "Karten werden geladen…",
    noCards: "Noch keine erkannten Karten in dieser Sitzung.",
    variantNote: "Exemplare einer Karte im selben Zustand werden eine Variante, zum höchsten ihrer Preise.",
  },
  result: {
    title: "Für {name} exportiert",
    showFile: "Datei zeigen",
    done: "Fertig",
    products: { one: "{count} Produkt", other: "{count} Produkte" },
    variants: { one: "{count} Variante", other: "{count} Varianten" },
    productsVariants: "{products} ({variants})",
    saved: "{what} gespeichert in <path></path>.",
    withoutPictures: {
      one: "{count} ohne Bilder: Füge sie vor dem Veröffentlichen in {name} hinzu.",
      other: "{count} ohne Bilder: Füge sie vor dem Veröffentlichen in {name} hinzu.",
    },
    leftOut: "Ausgelassen:",
  },
};
export default messages;
