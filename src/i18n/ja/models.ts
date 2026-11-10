import type { Messages } from "..";
import type en from "../en/models";

const messages: Messages<typeof en> = {
  picker: {
    model: "モデル",
    downloaded: "ダウンロード済み",
  },
  card: {
    sizes: "ダウンロード {size} · メモリ 約{memory}",
    modelPage: "モデルのページ",
    unsupported: "このビルドのCardloaderはローカルモデルに対応していません。",
    progress: "{done}/{size}（{percent}%）",
    left: "残り約{time}",
    ready: "ダウンロード済み · このパソコン上で動作し、インターネット接続は不要です",
    accept: "<license>{license}</license>に同意します",
    licenseNote:
      "このモデルは{maker}が作成し、Hugging Faceからダウンロードされます。Cardloaderの一部ではありません。同意する前にライセンスをお読みください。",
    licenseLimit: "ライセンスでは、一定の収益額未満の場合にのみ商用利用が認められています。",
    resume: "ダウンロードを再開",
    download: "ダウンロード",
    partial: "{size}のうち{done}をダウンロード済み",
  },
  device: {
    runOn: "実行するデバイス",
    auto: "自動",
    noGpu: "GPUが見つからないためCPU",
    cpu: "CPUのみ",
    integrated: "{backend} · 内蔵",
    missing: "{name}（見つかりません：CPUを使用）",
    tooSmall: "{model}には約{need}が必要ですが、{gpu}は{have}です。収まらない部分はCPUで実行されるため、遅くなります。",
    loaded: "読み込まれたモデルは{runsOn}で実行されています。",
    partly:
      "読み込まれたモデルは{layers}層のうち{gpu}層を{runsOn}で、残りをCPUで実行しています。GPUの空きメモリが足りませんでした。",
    fellBack: "GPUでモデルを読み込めなかったため、CPUで実行しています（{error}）。",
  },
  compare: {
    title: "ローカルモデルと比較",
    stop: "停止",
    intro:
      "Ximilarが「{name}」で識別した各カードをローカルモデルが読み取り、2つの答えを比較します。セッションの内容は変更されません。複数のモデルで実行すると、どれがあなたのカードに合うかがわかります。",
    downloadFirst: "先に設定でローカルモデルをダウンロードしてください。",
    model: "モデル",
    compareAgain: "もう一度比較",
    compare: "比較",
    sameName: "名前が一致",
    sameNumber: "番号が一致",
    perCard: "カードごと",
    loadingModel: "モデルを読み込み中…",
    progress: { other: "{done}/{count}枚" },
    of: "{n}/{total}",
    ofWithNumber: "番号のある{total}枚中{n}枚",
    onThisComputer: "このパソコン上",
    localModel: "ローカルモデル",
    name: "名前",
    number: "番号",
    nothingRead: "読み取りなし",
    noCards: "このセッションには、比較できるXimilarで識別済みのカードがありません。",
    same: "一致",
    different: "不一致",
  },
};
export default messages;
