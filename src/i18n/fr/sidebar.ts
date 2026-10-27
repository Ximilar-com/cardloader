import type { Messages } from "..";
import type en from "../en/sidebar";

const messages: Messages<typeof en> = {
  nav: {
    home: "Accueil",
    sessions: "Sessions",
    cards: "Cartes",
    settings: "Réglages",
  },
  unsavedSettings: "Réglages non enregistrés",
  poweredBy: "Propulsé par Ximilar",
  toast: {
    noneUpsideDown: "Aucune de ces cartes n’est à l’envers",
    turned: { one: "{count} carte remise à l’endroit", other: "{count} cartes remises à l’endroit" },
    turnedSkipped: {
      one: "{count} carte remise à l’endroit · {skipped} n’étaient pas à l’envers",
      other: "{count} cartes remises à l’endroit · {skipped} n’étaient pas à l’envers",
    },
  },
};
export default messages;
