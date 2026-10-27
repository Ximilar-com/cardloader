import type { Messages } from "..";
import type en from "../en/market";

const messages: Messages<typeof en> = {
  conditionHint: {
    excellent: "lightly played",
    veryGood: "moderately played",
    poor: "heavily played",
  },
  priced: {
    allCards: "Tutte le carte",
    canBeListed: {
      one: "{count} carta può essere messa in vendita",
      other: "{count} carte possono essere messe in vendita",
    },
    canBeExported: { one: "{count} carta può essere esportata", other: "{count} carte possono essere esportate" },
    adjustPrices: "Regola i prezzi",
    priceUsd: "Prezzo (USD)",
    percentOfMarket: "<field></field> % del prezzo di mercato",
    percentLabel: "Percentuale del prezzo di mercato",
    endIn99: "finale .99",
    atLeast: "almeno $<field></field>",
    lowestPrice: "Prezzo minimo",
    apply: "Applica alle carte scelte",
    choose: "Scegli #{seq}",
    price: "Prezzo",
    priceOf: "Prezzo di #{seq}",
  },
  listingType: {
    auction: "Asta",
    auctionHint: "il prezzo è l’offerta iniziale",
    buyItNow: "Compralo Subito",
    buyItNowHint: "prezzo fisso",
  },
  shopifyFile: {
    products: "Prodotti",
    productsHint: "nuovi prodotti con varianti",
    inventory: "Aggiornamento inventario",
    inventoryHint: "prodotti esistenti",
  },
  shopifyGrouping: {
    byCard: "Un prodotto per carta",
    byCardHint: "le condizioni sono varianti",
    perCard: "Un prodotto per carta scansionata",
  },
  export: {
    title: "Esporta per {name}",
    uploadingProgress: "Caricamento foto… {done}/{total}",
    preparingProgress: "Preparazione… {done}/{total}",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} senza prezzo", other: "{count} senza prezzo" },
    stop: "Ferma",
    exporting: "Esportazione…",
    exportCsv: "Esporta CSV…",
    tcgplayerIntro:
      "Un elenco delle carte collezionabili non gradate nei termini di TCGplayer — linea di prodotto, set, nome, numero, condizione e il tuo prezzo — con il link al prodotto TCGplayer di ogni carta. TCGplayer importa solo file esportati dal proprio Seller Portal, quindi aggiungi le carte in Seller Portal partendo da questo elenco. Lì le carte gradate si vendono come Listings with Photos, una alla volta, e TCGplayer non ha carte sportive.",
    shopifyInventoryIntro:
      "Il file di inventario di Shopify (Products → Inventory → Import): imposta le scorte di ogni prodotto nella sede al numero di copie presenti qui. Trova i prodotti creati dall’esportazione prodotti di questa sessione — con lo stesso raggruppamento — tramite i loro handle e condizioni.",
    shopifyProductsIntro:
      "Il file prodotti di Shopify (Products → Import). Le carte vengono ritrovate tramite i loro handle, quindi spunta “Overwrite products with matching handles” per aggiornare i prodotti importati in precedenza. Gli slab gradati sono prodotti a sé. Nei negozi con più sedi, imposta poi le scorte con l’aggiornamento inventario.",
    whatnotIntro:
      "Il file di inserzioni in blocco di Whatnot: importalo in Seller Hub (Inventory → Import from CSV) per avere bozze di inserzioni, oppure in uno show (Add → Create Temporary Listing → Upload CSV). Gli slab gradati vengono messi in vendita come Graded, con ente di gradazione e voto nel titolo e nella descrizione. I prezzi sono in dollari interi, arrotondati per eccesso.",
    type: "Tipo",
    listingType: "Tipo di inserzione",
    shippingProfile: "Profilo di spedizione",
    file: "File",
    shopifyFile: "File Shopify",
    products: "Prodotti",
    condition: "Condizione delle carte non gradate",
    conditionNote: "Le carte di cui Ximilar ha valutato la condizione mantengono la loro.",
    location: "Sede",
    locationPlaceholder: "ad es. Sede negozio",
    locationNote: "Esattamente come si chiama in Shopify (Settings → Locations).",
    uploadPictures: "Carica le foto (ImgBB)",
    publishActive: "Pubblica (attive) invece delle bozze",
    offerable: "Gli acquirenti possono fare offerte",
    noImgbbKey:
      "{name} scarica le foto da link. Aggiungi una chiave API di ImgBB in Impostazioni per caricarle; senza, aggiungi le foto in {name}.",
    loadingCards: "Caricamento carte…",
    noCards: "Ancora nessuna carta identificata in questa sessione.",
    variantNote: "Le copie di una carta nella stessa condizione diventano una variante, al più alto dei loro prezzi.",
  },
  result: {
    title: "Esportato per {name}",
    showFile: "Mostra file",
    done: "Fatto",
    products: { one: "{count} prodotto", other: "{count} prodotti" },
    variants: { one: "{count} variante", other: "{count} varianti" },
    productsVariants: "{products} ({variants})",
    saved: "Salvato in <path></path>: {what}.",
    withoutPictures: {
      one: "{count} senza foto: aggiungi le foto in {name} prima di pubblicare.",
      other: "{count} senza foto: aggiungi le foto in {name} prima di pubblicare.",
    },
    leftOut: "Escluse:",
  },
};
export default messages;
