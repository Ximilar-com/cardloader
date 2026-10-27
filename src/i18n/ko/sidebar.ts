import type { Messages } from "..";
import type en from "../en/sidebar";

const messages: Messages<typeof en> = {
  nav: {
    home: "홈",
    sessions: "세션",
    cards: "카드",
    settings: "설정",
  },
  unsavedSettings: "저장되지 않은 설정",
  poweredBy: "Ximilar 제공",
  update: "업데이트",
  updateAvailable: "Cardloader {version}을(를) 사용할 수 있습니다",
  toast: {
    noneUpsideDown: "거꾸로 된 카드가 없습니다",
    turned: { other: "카드 {count}장을 똑바로 돌렸습니다" },
    turnedSkipped: {
      other: "카드 {count}장을 똑바로 돌렸습니다 · {skipped}장은 거꾸로 되어 있지 않았습니다",
    },
  },
};
export default messages;
