import type { Messages } from "..";
import type en from "../en/ebay";

const messages: Messages<typeof en> = {
  status: {
    draft: "초안",
    listed: "등록됨",
    sold: "판매됨",
    ended: "종료됨",
  },
  badge: {
    draft: "eBay: 초안",
    listed: "eBay: 등록됨",
    sold: "eBay: 판매됨",
    ended: "eBay: 종료됨",
  },
  section: {
    notOnEbay: "eBay에 없음",
    listing: "리스팅 {id}",
    draftNote: "게시되지 않은 리스팅입니다. 게시하거나 eBay에서 편집하세요.",
    publish: "게시",
    endListing: "리스팅 종료",
    remove: "eBay에서 제거",
  },
  list: {
    title: "eBay에 등록",
    connectFirst: "먼저 eBay 계정을 연결하세요.",
    policiesFirst: "먼저 배송, 결제, 반품 정책과 카드 발송지를 선택하세요.",
    total: "{cards} · {total}",
    withoutPrice: { other: "가격 없음 {count}장" },
    starting: "시작 중…",
    createDrafts: { other: "초안 {count}개 만들기" },
    intro:
      "각 카드는 사진과 함께 고정 가격의 게시되지 않은 리스팅(초안)으로 eBay에 만들어집니다. 초안을 확인한 다음 세션에서 — 또는 eBay에서 — 게시하세요. 가격은 Ximilar에 있으면 Ximilar의 가격을 사용합니다.",
    introSandbox:
      "각 카드는 사진과 함께 고정 가격의 게시되지 않은 리스팅(초안)으로 eBay 샌드박스에 만들어집니다. 초안을 확인한 다음 세션에서 — 또는 eBay에서 — 게시하세요. 가격은 Ximilar에 있으면 Ximilar의 가격을 사용합니다.",
    condition: "그레이딩되지 않은 카드의 상태",
    gradedNote: "그레이딩 슬랩에 든 카드는 감정 회사와 등급을 포함해 등록됩니다.",
    loadingCards: "카드 불러오는 중…",
    noCards: "이 세션에 아직 식별된 카드가 없습니다.",
  },
  note: {
    sold: "eBay에서 판매됨",
    live: "eBay에 공개 중 — 새 가격이 리스팅에 적용됩니다 · {note}",
    draft: "이미 eBay에 초안이 있습니다 — 다시 등록하면 업데이트됩니다 · {note}",
  },
};
export default messages;
