import type { Messages } from "..";
import type en from "../en/sidebar";

const messages: Messages<typeof en> = {
  nav: {
    home: "Início",
    sessions: "Sessões",
    cards: "Cartas",
    settings: "Configurações",
  },
  unsavedSettings: "Configurações não salvas",
  poweredBy: "Com tecnologia Ximilar",
  toast: {
    noneUpsideDown: "Nenhuma destas cartas está de cabeça para baixo",
    turned: { one: "{count} carta endireitada", other: "{count} cartas endireitadas" },
    turnedSkipped: {
      one: "{count} carta endireitada · {skipped} não estavam de cabeça para baixo",
      other: "{count} cartas endireitadas · {skipped} não estavam de cabeça para baixo",
    },
  },
};
export default messages;
