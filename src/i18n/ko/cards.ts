import type { Messages } from "..";
import type en from "../en/cards";

const messages: Messages<typeof en> = {
  title: "카드",
  total: { other: "카드 <b>{count}</b>장" },
  intro: "이 컴퓨터에 있는 모든 세션의 식별된 카드입니다. 이름, 세트, 번호, 위치, 메모로 검색할 수 있습니다.",
  filter: {
    all: "전체",
    review: "검토 필요",
    graded: "그레이딩됨",
    duplicates: "중복",
    agent: "AI 검토",
  },
  selected: "{count}장 선택됨",
  accept: "승인",
  accepted: { other: "일치 항목 {count}개를 승인했습니다" },
  unaccept: "승인 취소",
  unacceptTitle: "승인 되돌리기",
  unaccepted: { other: "일치 항목 {count}개의 승인을 취소했습니다" },
  setLocation: "위치 지정…",
  turnUpright: "똑바로 돌리기",
  turnUprightTitle: "Ximilar가 거꾸로 찾은 카드를 앞면과 뒷면 모두 똑바로 돌립니다",
  clearSelection: "선택 해제",
  searchPlaceholder: "이름, 세트, 번호, 위치…",
  search: "카드 검색",
  showingNewest: "최신 {count}장 표시 — 나머지는 검색이나 필터로 찾으세요.",
  noMatch: "일치하는 카드가 없습니다.",
  empty: "아직 식별된 카드가 없습니다.",
  goToSessions: "세션으로 이동",
  position: "카드 {count}장 중 {at}번째",
  turned: "똑바로 돌림",
  inSession: "세션 “{name}”",
  location: {
    title: { other: "카드 {count}장의 위치" },
    hint: "보관 장소(예: “상자 4” 또는 “바인더 2, 7쪽”). 검색할 수 있고 CSV 내보내기에 포함됩니다.",
    confirm: "위치 지정",
    done: { other: "카드 {count}장의 위치: {location}" },
  },
};
export default messages;
