import type { Messages } from "..";
import type en from "../en/market";

const messages: Messages<typeof en> = {
  conditionHint: {
    excellent: "약간 사용감(LP)",
    veryGood: "중간 사용감(MP)",
    poor: "심한 사용감(HP)",
  },
  priced: {
    allCards: "모든 카드",
    canBeListed: { other: "카드 {count}장 등록 가능" },
    canBeExported: { other: "카드 {count}장 내보내기 가능" },
    adjustPrices: "가격 조정",
    priceUsd: "가격(USD)",
    percentOfMarket: "시세의 <field></field> %",
    percentLabel: "시세 대비 비율",
    endIn99: ".99로 끝내기",
    atLeast: "최소 $<field></field>",
    lowestPrice: "최저 가격",
    apply: "선택한 카드에 적용",
    choose: "#{seq} 선택",
    price: "가격",
    priceOf: "#{seq} 가격",
  },
  listingType: {
    auction: "경매",
    auctionHint: "가격이 시작 입찰가가 됩니다",
    buyItNow: "즉시 구매",
    buyItNowHint: "고정 가격",
  },
  shopifyFile: {
    products: "상품",
    productsHint: "변형이 있는 새 상품",
    inventory: "재고 업데이트",
    inventoryHint: "기존 상품",
  },
  shopifyGrouping: {
    byCard: "카드당 상품 하나",
    byCardHint: "상태가 변형이 됩니다",
    perCard: "스캔한 카드마다 상품 하나",
  },
  export: {
    title: "{name}용 내보내기",
    uploadingProgress: "사진 업로드 중… {done}/{total}",
    preparingProgress: "준비 중… {done}/{total}",
    total: "{cards} · {total}",
    withoutPrice: { other: "가격 없음 {count}장" },
    stop: "중지",
    exporting: "내보내는 중…",
    exportCsv: "CSV 내보내기…",
    tcgplayerIntro:
      "그레이딩되지 않은 트레이딩 카드를 TCGplayer 용어(제품 라인, 세트, 이름, 번호, 상태, 내 가격)로 정리한 목록이며, 각 카드의 TCGplayer 제품 링크가 포함됩니다. TCGplayer는 자체 Seller Portal에서 내보낸 파일만 가져오므로 이 목록을 보고 Seller Portal에서 카드를 추가하세요. 그레이딩된 카드는 그곳에서 Listings with Photos로 한 장씩 판매하며, TCGplayer에는 스포츠 카드가 없습니다.",
    shopifyInventoryIntro:
      "Shopify 재고 파일(Products → Inventory → Import): 각 상품의 해당 위치 재고를 여기의 사본 수로 설정합니다. 이 세션의 상품 내보내기로 만든 상품을 같은 그룹화 기준으로, 핸들과 상태를 통해 찾습니다.",
    shopifyProductsIntro:
      "Shopify 상품 파일(Products → Import). 카드는 핸들로 다시 찾으므로, 이전에 가져온 상품을 업데이트하려면 “Overwrite products with matching handles”를 체크하세요. 그레이딩 슬랩은 별도 상품이 됩니다. 위치가 여러 개인 스토어에서는 그다음 재고 업데이트로 재고를 설정하세요.",
    whatnotIntro:
      "Whatnot 대량 등록 파일: Seller Hub(Inventory → Import from CSV)에서 가져오면 초안 리스팅이 되고, 쇼에서는 Add → Create Temporary Listing → Upload CSV로 가져옵니다. 그레이딩 슬랩은 제목과 설명에 감정 회사와 등급을 넣어 Graded로 등록됩니다. 가격은 달러 단위로 올림합니다.",
    type: "유형",
    listingType: "리스팅 유형",
    shippingProfile: "배송 프로필",
    file: "파일",
    shopifyFile: "Shopify 파일",
    products: "상품",
    condition: "그레이딩되지 않은 카드의 상태",
    conditionNote: "Ximilar가 상태를 평가한 카드는 그 상태를 유지합니다.",
    location: "위치",
    locationPlaceholder: "예: 매장 위치",
    locationNote: "Shopify에 있는 이름 그대로 입력하세요(Settings → Locations).",
    uploadPictures: "사진 업로드(ImgBB)",
    publishActive: "초안 대신 게시(활성)",
    offerable: "구매자 가격 제안 허용",
    noImgbbKey:
      "{name}은(는) 링크에서 사진을 내려받습니다. 업로드하려면 설정에서 ImgBB API 키를 추가하세요. 키가 없으면 {name}에서 사진을 추가하세요.",
    loadingCards: "카드 불러오는 중…",
    noCards: "이 세션에 아직 식별된 카드가 없습니다.",
    variantNote: "같은 상태인 카드 사본은 그중 가장 높은 가격으로 하나의 변형이 됩니다.",
  },
  result: {
    title: "{name}용으로 내보냄",
    showFile: "파일 보기",
    done: "완료",
    products: { other: "상품 {count}개" },
    variants: { other: "변형 {count}개" },
    productsVariants: "{products}({variants})",
    saved: "{what}을(를) <path></path>에 저장했습니다.",
    withoutPictures: {
      other: "사진 없음 {count}장: 게시하기 전에 {name}에서 추가하세요.",
    },
    leftOut: "제외됨:",
  },
};
export default messages;
