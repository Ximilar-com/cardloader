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
      "このモデルはLiquid AIが作成し、Hugging Faceからダウンロードされます。Cardloaderの一部ではありません。ライセンスでは、一定の収益額未満の場合にのみ商用利用が認められています。同意する前にお読みください。",
    resume: "ダウンロードを再開",
    download: "ダウンロード",
    partial: "{size}のうち{done}をダウンロード済み",
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
