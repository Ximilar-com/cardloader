import type { Messages } from "..";
import type en from "../en/cards";

const messages: Messages<typeof en> = {
  title: "カード",
  total: { other: "<b>{count}</b>枚" },
  intro: "このパソコン上の全セッションで識別されたカードです。名前、セット、番号、保管場所、メモで検索できます。",
  filter: {
    all: "すべて",
    review: "要確認",
    graded: "グレード済み",
    duplicates: "重複",
    agent: "AIによる確認",
  },
  selected: "{count}件を選択中",
  accept: "承認",
  accepted: { other: "{count}件の一致を承認しました" },
  unaccept: "承認を取り消す",
  unacceptTitle: "承認を取り消します",
  unaccepted: { other: "{count}件の一致の承認を取り消しました" },
  setLocation: "保管場所を設定…",
  turnUpright: "正しい向きにする",
  turnUprightTitle: "Ximilarが上下逆と判定したカードを、表面・裏面とも正しい向きにします",
  clearSelection: "選択を解除",
  searchPlaceholder: "名前、セット、番号、保管場所…",
  search: "カードを検索",
  showingNewest: "新しい順に{count}枚を表示中 — ほかのカードは検索または絞り込みで探してください。",
  noMatch: "一致するカードはありません。",
  empty: "識別済みのカードはまだありません。",
  goToSessions: "セッションへ移動",
  position: "カード {at}/{count}",
  turned: "正しい向きにしました",
  inSession: "セッション「{name}」",
  location: {
    title: { other: "{count}枚のカードの保管場所" },
    hint: "保管している場所です（例：「箱4」「バインダー2、7ページ」）。検索でき、CSVエクスポートにも含まれます。",
    confirm: "保管場所を設定",
    done: { other: "{count}枚のカードを{location}に設定しました" },
  },
};
export default messages;
