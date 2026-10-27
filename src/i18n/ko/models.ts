import type { Messages } from "..";
import type en from "../en/models";

const messages: Messages<typeof en> = {
  picker: {
    model: "모델",
    downloaded: "다운로드됨",
  },
  card: {
    sizes: "다운로드 {size} · 메모리 약 {memory}",
    modelPage: "모델 페이지",
    unsupported: "이 cardloader 빌드는 로컬 모델을 지원하지 않습니다.",
    progress: "{size} 중 {done}({percent}%)",
    left: "약 {time} 남음",
    ready: "다운로드됨 · 이 컴퓨터에서 실행되며 인터넷이 필요 없습니다",
    accept: "<license>{license}</license>에 동의합니다",
    licenseNote:
      "이 모델은 Liquid AI가 만들었고 Hugging Face에서 다운로드되며, cardloader의 일부가 아닙니다. 라이선스는 일정 매출 이하에서만 상업적 사용을 허용합니다 — 동의하기 전에 읽어 보세요.",
    resume: "다운로드 재개",
    download: "다운로드",
    partial: "{size} 중 {done} 다운로드됨",
  },
  compare: {
    title: "로컬 모델과 비교",
    stop: "중지",
    intro:
      "로컬 모델이 “{name}”에서 Ximilar가 식별한 각 카드를 읽고 두 결과를 비교합니다. 세션은 아무것도 바뀌지 않습니다. 여러 모델로 실행해 내 카드에 맞는 모델을 찾아보세요.",
    downloadFirst: "먼저 설정에서 로컬 모델을 다운로드하세요.",
    model: "모델",
    compareAgain: "다시 비교",
    compare: "비교",
    sameName: "같은 이름",
    sameNumber: "같은 번호",
    perCard: "카드별",
    loadingModel: "모델 불러오는 중…",
    progress: { other: "카드 {count}장 중 {done}장" },
    of: "{total}장 중 {n}장",
    ofWithNumber: "번호가 있는 {total}장 중 {n}장",
    onThisComputer: "이 컴퓨터에서",
    localModel: "로컬 모델",
    name: "이름",
    number: "번호",
    nothingRead: "읽은 내용 없음",
    noCards: "이 세션에는 비교할 Ximilar 식별 카드가 없습니다.",
    same: "같음",
    different: "다름",
  },
};
export default messages;
