import type { Messages } from "..";
import type en from "../en/ebay";

const messages: Messages<typeof en> = {
  status: {
    draft: "Bozza",
    listed: "In vendita",
    sold: "Venduta",
    ended: "Terminata",
  },
  badge: {
    draft: "eBay: bozza",
    listed: "eBay: in vendita",
    sold: "eBay: venduta",
    ended: "eBay: terminata",
  },
  section: {
    notOnEbay: "Non su eBay",
    listing: "Inserzione {id}",
    draftNote: "Un’inserzione non pubblicata: pubblicala o modificala su eBay.",
    publish: "Pubblica",
    endListing: "Termina inserzione",
    remove: "Rimuovi da eBay",
  },
  list: {
    title: "Metti in vendita su eBay",
    connectFirst: "Prima collega il tuo account eBay.",
    policiesFirst: "Prima scegli le regole di spedizione, pagamento e reso e da dove partono le carte.",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} senza prezzo", other: "{count} senza prezzo" },
    starting: "Avvio…",
    createDrafts: { one: "Crea {count} bozza", other: "Crea {count} bozze" },
    intro:
      "Ogni carta diventa un’inserzione non pubblicata (una bozza) su eBay, con le sue foto, a prezzo fisso. Controlla le bozze, poi pubblicale dalla sessione — o su eBay. I prezzi vengono da Ximilar quando ne ha uno.",
    introSandbox:
      "Ogni carta diventa un’inserzione non pubblicata (una bozza) nella sandbox di eBay, con le sue foto, a prezzo fisso. Controlla le bozze, poi pubblicale dalla sessione — o su eBay. I prezzi vengono da Ximilar quando ne ha uno.",
    condition: "Condizione delle carte non gradate",
    gradedNote: "Le carte in uno slab gradato vengono messe in vendita con l’ente di gradazione e il voto.",
    loadingCards: "Caricamento carte…",
    noCards: "Ancora nessuna carta identificata in questa sessione.",
  },
  note: {
    sold: "Venduta su eBay",
    live: "In vendita su eBay — un nuovo prezzo si applica all’inserzione · {note}",
    draft: "Già in bozza su eBay — rimetterla in vendita la aggiorna · {note}",
  },
};
export default messages;
