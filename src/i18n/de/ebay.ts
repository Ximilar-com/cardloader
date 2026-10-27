import type { Messages } from "..";
import type en from "../en/ebay";

const messages: Messages<typeof en> = {
  status: {
    draft: "Entwurf",
    listed: "Gelistet",
    sold: "Verkauft",
    ended: "Beendet",
  },
  badge: {
    draft: "eBay: Entwurf",
    listed: "eBay: gelistet",
    sold: "eBay: verkauft",
    ended: "eBay: beendet",
  },
  section: {
    notOnEbay: "Nicht auf eBay",
    listing: "Angebot {id}",
    draftNote: "Ein unveröffentlichtes Angebot: Veröffentliche es oder bearbeite es auf eBay.",
    publish: "Veröffentlichen",
    endListing: "Angebot beenden",
    remove: "Von eBay entfernen",
  },
  list: {
    title: "Auf eBay anbieten",
    connectFirst: "Verbinde zuerst dein eBay-Konto.",
    policiesFirst:
      "Wähle zuerst deine Versand-, Zahlungs- und Rückgaberichtlinien und von wo die Karten versendet werden.",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} ohne Preis", other: "{count} ohne Preis" },
    starting: "Wird gestartet…",
    createDrafts: { one: "{count} Entwurf erstellen", other: "{count} Entwürfe erstellen" },
    intro:
      "Jede Karte wird ein unveröffentlichtes Angebot (ein Entwurf) auf eBay, mit ihren Bildern, zum Festpreis. Prüfe die Entwürfe und veröffentliche sie dann aus der Sitzung — oder auf eBay. Die Preise kommen von Ximilar, wo es einen hat.",
    introSandbox:
      "Jede Karte wird ein unveröffentlichtes Angebot (ein Entwurf) in eBays Sandbox, mit ihren Bildern, zum Festpreis. Prüfe die Entwürfe und veröffentliche sie dann aus der Sitzung — oder auf eBay. Die Preise kommen von Ximilar, wo es einen hat.",
    condition: "Zustand unbewerteter Karten",
    gradedNote: "Karten in einem bewerteten Slab werden mit Grader und Bewertung angeboten.",
    loadingCards: "Karten werden geladen…",
    noCards: "Noch keine erkannten Karten in dieser Sitzung.",
  },
  note: {
    sold: "Auf eBay verkauft",
    live: "Live auf eBay — ein neuer Preis gilt für das Angebot · {note}",
    draft: "Schon ein Entwurf auf eBay — erneutes Anbieten aktualisiert ihn · {note}",
  },
};
export default messages;
