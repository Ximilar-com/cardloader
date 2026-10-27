import type { Messages } from "..";
import type en from "../en/cards";

const messages: Messages<typeof en> = {
  title: "Karten",
  total: { one: "<b>{count}</b> Karte", other: "<b>{count}</b> Karten" },
  intro:
    "Alle erkannten Karten deiner Sitzungen auf diesem Computer. Suche nach Name, Set, Nummer, Lagerort oder Notizen.",
  filter: {
    all: "Alle",
    review: "Zu prüfen",
    graded: "Bewertet",
    duplicates: "Duplikate",
    agent: "Von KI",
  },
  selected: "{count} ausgewählt",
  accept: "Übernehmen",
  accepted: { one: "{count} Treffer übernommen", other: "{count} Treffer übernommen" },
  unaccept: "Zurücknehmen",
  unacceptTitle: "Übernahme zurücknehmen",
  unaccepted: { one: "{count} Übernahme zurückgenommen", other: "{count} Übernahmen zurückgenommen" },
  setLocation: "Lagerort festlegen…",
  turnUpright: "Aufrichten",
  turnUprightTitle: "Karten, die Ximilar auf dem Kopf fand, mit Vorder- und Rückseite drehen",
  clearSelection: "Auswahl aufheben",
  searchPlaceholder: "Name, Set, Nummer, Lagerort…",
  search: "Karten suchen",
  showingNewest: "Die neuesten {count} werden gezeigt — suche oder filtere, um die anderen zu finden.",
  noMatch: "Keine passenden Karten.",
  empty: "Noch keine erkannten Karten.",
  goToSessions: "Zu den Sitzungen",
  position: "Karte {at} von {count}",
  turned: "Aufgerichtet",
  inSession: "In Sitzung „{name}“",
  location: {
    title: { one: "Lagerort von {count} Karte", other: "Lagerort von {count} Karten" },
    hint: "Wo sie liegen, z. B. „Box 4“ oder „Ordner 2, Seite 7“. Danach kann man suchen, und er kommt in den CSV-Export.",
    confirm: "Lagerort festlegen",
    done: { one: "{count} Karte jetzt in {location}", other: "{count} Karten jetzt in {location}" },
  },
};
export default messages;
