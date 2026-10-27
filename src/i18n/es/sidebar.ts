import type { Messages } from "..";
import type en from "../en/sidebar";

const messages: Messages<typeof en> = {
  nav: {
    home: "Inicio",
    sessions: "Sesiones",
    cards: "Cartas",
    settings: "Ajustes",
  },
  unsavedSettings: "Ajustes sin guardar",
  poweredBy: "Con la tecnología de Ximilar",
  update: "Actualizar",
  updateAvailable: "Cardloader {version} está disponible",
  toast: {
    noneUpsideDown: "Ninguna de estas cartas está boca abajo",
    turned: { one: "{count} carta enderezada", other: "{count} cartas enderezadas" },
    turnedSkipped: {
      one: "{count} carta enderezada · {skipped} no estaban boca abajo",
      other: "{count} cartas enderezadas · {skipped} no estaban boca abajo",
    },
  },
};
export default messages;
