import type { Messages } from "..";
import type en from "../en/wizard";

const messages: Messages<typeof en> = {
  title: "Cardloader 설정",
  progress: "{steps}단계 중 {step}단계 · 모든 항목은 나중에 설정에서 바꿀 수 있습니다",
  alwaysIncluded: "항상 포함",
  language: {
    title: "언어 선택",
    text: "지금부터 Cardloader가 이 언어로 표시됩니다.",
  },
  interests: {
    title: "스캔할 카드",
    text: "수집하는 항목을 모두 선택하세요.",
    tcg: "트레이딩 카드 게임",
    tcgHint: "Pokémon, Magic, Yu-Gi-Oh!, One Piece, Lorcana…",
    sport: "스포츠 카드",
    sportHint: "야구, 농구, 미식축구, 하키, 축구…",
  },
  games: {
    title: "게임 선택",
    text: "하나를 고르면 모든 새 세션이 그 게임으로 설정되어 Ximilar가 그 게임에서만 일치 항목을 찾습니다. 여러 개를 고르거나 하나도 고르지 않으면 세션마다 선택합니다.",
  },
  goals: {
    title: "알고 싶은 정보",
    text: "필요한 항목을 모두 선택하세요.",
    identification: "식별",
    identificationHint: "모든 카드의 이름, 세트, 번호",
    grading: "그레이딩",
    gradingHint: "코너, 엣지, 표면, 센터링까지 보는 전체 등급",
    condition: "상태",
    conditionHint: "리스팅용 빠른 상태 판정(예: Near Mint)",
    prices: "가격",
    pricesHint: "각 카드의 시세",
  },
  sources: {
    title: "카드를 가져올 방법",
    text: "사용하는 방법을 모두 선택하세요. 첫 번째 방법으로 새 세션이 열립니다.",
    scanner: "스캐너",
    scannerHint: "급지 장치가 있는 문서 스캐너(예: RICOH fi-8170)",
    folder: "이미지 폴더",
    folderHint: "이 컴퓨터에 이미 있는 사진 또는 스캔",
    watchFolder: "감시 폴더",
    watchFolderHint: "폴더에 새 이미지가 들어오는 대로 카드를 분석합니다",
    csv: "CSV 파일",
    csvHint: "카드 또는 이미지 경로 목록",
  },
  engine: {
    title: "카드 식별 방법",
    ximilar: "Ximilar API",
    ximilarHint: "정확한 식별, 가격, 그레이딩. API 토큰과 인터넷 연결이 필요합니다.",
    local: "로컬 AI 모델",
    localHint:
      "이 컴퓨터에서 오프라인으로 무료로 실행됩니다. 이름, 번호, 세트를 읽고 상태를 추정하지만 정확도는 낮습니다.",
    token: "API 토큰",
    tokenSaved: "이미 저장됨. 유지하려면 비워 두세요",
    tokenPaste: "토큰을 붙여 넣으세요",
    tokenHelp: "<link>Ximilar App</link>의 프로필에서 찾을 수 있습니다. 나중에 설정에서 추가해도 됩니다.",
    pricesNeedApi: "가격을 보려면 Ximilar API가 필요합니다.",
    roughGrades: "로컬 모델의 등급과 상태는 작은 모델의 대략적인 추정치이며, 감정사의 평가가 아닙니다.",
  },
};
export default messages;
