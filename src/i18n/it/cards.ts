import type { Messages } from "..";
import type en from "../en/cards";

const messages: Messages<typeof en> = {
  title: "Carte",
  total: { one: "<b>{count}</b> carta", other: "<b>{count}</b> carte" },
  intro:
    "Tutte le carte identificate delle tue sessioni su questo computer. Cerca per nome, set, numero, posizione o note.",
  filter: {
    all: "Tutte",
    review: "Da rivedere",
    graded: "Gradate",
    duplicates: "Doppioni",
    agent: "Dall’IA",
  },
  selected: "{count} selezionate",
  accept: "Accetta",
  accepted: { one: "{count} corrispondenza accettata", other: "{count} corrispondenze accettate" },
  unaccept: "Revoca",
  unacceptTitle: "Revoca le accettazioni",
  unaccepted: {
    one: "Accettazione revocata per {count} corrispondenza",
    other: "Accettazione revocata per {count} corrispondenze",
  },
  setLocation: "Imposta posizione…",
  turnUpright: "Raddrizza",
  turnUprightTitle: "Raddrizza le carte che Ximilar ha trovato capovolte, fronte e retro",
  clearSelection: "Annulla la selezione",
  searchPlaceholder: "Nome, set, numero, posizione…",
  search: "Cerca carte",
  showingNewest: "Sono mostrate le {count} più recenti — cerca o filtra per trovare le altre.",
  noMatch: "Nessuna carta corrisponde.",
  empty: "Ancora nessuna carta identificata.",
  goToSessions: "Vai alle sessioni",
  position: "Carta {at} di {count}",
  turned: "Raddrizzata",
  inSession: "Nella sessione “{name}”",
  location: {
    title: { one: "Posizione di {count} carta", other: "Posizione di {count} carte" },
    hint: "Dove sono conservate, ad es. “Scatola 4” o “Raccoglitore 2, pagina 7”. Si può cercare e finisce nell’esportazione CSV.",
    confirm: "Imposta posizione",
    done: { one: "{count} carta ora in {location}", other: "{count} carte ora in {location}" },
  },
};
export default messages;
