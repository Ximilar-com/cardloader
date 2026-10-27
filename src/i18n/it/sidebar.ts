import type { Messages } from "..";
import type en from "../en/sidebar";

const messages: Messages<typeof en> = {
  nav: {
    home: "Home",
    sessions: "Sessioni",
    cards: "Carte",
    settings: "Impostazioni",
  },
  unsavedSettings: "Impostazioni non salvate",
  poweredBy: "Basato su Ximilar",
  update: "Aggiorna",
  updateAvailable: "Cardloader {version} è disponibile",
  toast: {
    noneUpsideDown: "Nessuna di queste carte è capovolta",
    turned: { one: "{count} carta raddrizzata", other: "{count} carte raddrizzate" },
    turnedSkipped: {
      one: "{count} carta raddrizzata · {skipped} non erano capovolte",
      other: "{count} carte raddrizzate · {skipped} non erano capovolte",
    },
  },
};
export default messages;
