import type { Messages } from "..";
import type en from "../en/ebay";

const messages: Messages<typeof en> = {
  status: {
    draft: "下書き",
    listed: "出品中",
    sold: "販売済み",
    ended: "終了",
  },
  badge: {
    draft: "eBay：下書き",
    listed: "eBay：出品中",
    sold: "eBay：販売済み",
    ended: "eBay：終了",
  },
  section: {
    notOnEbay: "eBay未出品",
    listing: "出品 {id}",
    draftNote: "未公開の出品です。公開するか、eBayで編集してください。",
    publish: "公開",
    endListing: "出品を終了",
    remove: "eBayから削除",
  },
  list: {
    title: "eBayに出品",
    connectFirst: "先にeBayアカウントを接続してください。",
    policiesFirst: "先に配送、支払い、返品のポリシーと発送元を選択してください。",
    total: "{cards} · {total}",
    withoutPrice: { other: "{count}枚は価格なし" },
    starting: "開始中…",
    createDrafts: { other: "{count}件の下書きを作成" },
    intro:
      "各カードは画像付き・固定価格の未公開の出品（下書き）としてeBayに作成されます。下書きを確認してから、セッションまたはeBayで公開してください。価格はXimilarに価格がある場合はそれを使います。",
    introSandbox:
      "各カードは画像付き・固定価格の未公開の出品（下書き）としてeBayのサンドボックスに作成されます。下書きを確認してから、セッションまたはeBayで公開してください。価格はXimilarに価格がある場合はそれを使います。",
    condition: "グレードなしカードのコンディション",
    gradedNote: "鑑定済みスラブのカードは、鑑定会社とグレード付きで出品されます。",
    loadingCards: "カードを読み込み中…",
    noCards: "このセッションには識別済みのカードがまだありません。",
  },
  note: {
    sold: "eBayで販売済み",
    live: "eBayで出品中 — 新しい価格が出品に反映されます · {note}",
    draft: "eBayに下書きがあります — もう一度出品すると更新されます · {note}",
  },
};
export default messages;
