import type { Messages } from "..";
import type en from "../en/sidebar";

const messages: Messages<typeof en> = {
  nav: {
    home: "Start",
    sessions: "Sitzungen",
    cards: "Karten",
    settings: "Einstellungen",
  },
  unsavedSettings: "Nicht gespeicherte Einstellungen",
  poweredBy: "Powered by Ximilar",
  update: "Update",
  updateAvailable: "Cardloader {version} ist verfügbar",
  toast: {
    noneUpsideDown: "Keine dieser Karten steht auf dem Kopf",
    turned: { one: "{count} Karte aufgerichtet", other: "{count} Karten aufgerichtet" },
    turnedSkipped: {
      one: "{count} Karte aufgerichtet · {skipped} standen nicht auf dem Kopf",
      other: "{count} Karten aufgerichtet · {skipped} standen nicht auf dem Kopf",
    },
  },
};
export default messages;
