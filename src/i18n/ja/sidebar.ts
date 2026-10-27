import type { Messages } from "..";
import type en from "../en/sidebar";

const messages: Messages<typeof en> = {
  nav: {
    home: "ホーム",
    sessions: "セッション",
    cards: "カード",
    settings: "設定",
  },
  unsavedSettings: "未保存の設定",
  poweredBy: "Powered by Ximilar",
  toast: {
    noneUpsideDown: "上下逆のカードはありません",
    turned: { other: "{count}枚のカードを正しい向きにしました" },
    turnedSkipped: {
      other: "{count}枚のカードを正しい向きにしました · {skipped}枚は上下逆ではありませんでした",
    },
  },
};
export default messages;
