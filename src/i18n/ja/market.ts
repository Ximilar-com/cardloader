import type { Messages } from "..";
import type en from "../en/market";

const messages: Messages<typeof en> = {
  conditionHint: {
    excellent: "Lightly Played相当",
    veryGood: "Moderately Played相当",
    poor: "Heavily Played相当",
  },
  priced: {
    allCards: "すべてのカード",
    canBeListed: { other: "{count}枚のカードを出品できます" },
    canBeExported: { other: "{count}枚のカードをエクスポートできます" },
    adjustPrices: "価格を調整",
    priceUsd: "価格（USD）",
    percentOfMarket: "相場の<field></field>%",
    percentLabel: "相場に対する割合（%）",
    endIn99: "末尾を.99にする",
    atLeast: "最低$<field></field>",
    lowestPrice: "最低価格",
    apply: "選択したカードに適用",
    choose: "#{seq}を選択",
    price: "価格",
    priceOf: "#{seq}の価格",
  },
  listingType: {
    auction: "オークション",
    auctionHint: "価格は開始価格になります",
    buyItNow: "即決",
    buyItNowHint: "固定価格",
  },
  shopifyFile: {
    products: "商品",
    productsHint: "バリエーション付きの新しい商品",
    inventory: "在庫の更新",
    inventoryHint: "既存の商品",
  },
  shopifyGrouping: {
    byCard: "カードごとに1商品",
    byCardHint: "コンディションはバリエーション",
    perCard: "スキャンしたカードごとに1商品",
  },
  export: {
    title: "{name}用にエクスポート",
    uploadingProgress: "画像をアップロード中… {done}/{total}",
    preparingProgress: "準備中… {done}/{total}",
    total: "{cards} · {total}",
    withoutPrice: { other: "{count}枚は価格なし" },
    stop: "停止",
    exporting: "エクスポート中…",
    exportCsv: "CSVをエクスポート…",
    tcgplayerIntro:
      "グレードなしのトレーディングカードを、TCGplayerの項目（プロダクトライン、セット、名前、番号、コンディション、あなたの価格）で一覧にし、各カードのTCGplayer商品リンクを付けます。TCGplayerは自社のSeller Portalでエクスポートしたファイルしかインポートできないため、この一覧を見ながらSeller Portalでカードを追加してください。グレード付きのカードはTCGplayerではListings with Photosとして1枚ずつ販売します。また、TCGplayerではスポーツカードは扱っていません。",
    shopifyInventoryIntro:
      "Shopifyの在庫ファイル（商品管理 → 在庫 → インポート）です。各商品のロケーションでの在庫数を、ここにある枚数に設定します。このセッションの商品エクスポートで作成した商品を（同じまとめ方で）ハンドルとコンディションから探します。",
    shopifyProductsIntro:
      "Shopifyの商品ファイル（商品管理 → インポート）です。カードはハンドルで照合されるため、以前にインポートした商品を更新するには「同じハンドルの商品を上書きする」にチェックを入れてください。鑑定済みスラブはそれぞれ単独の商品になります。複数のロケーションがあるストアでは、あとで在庫の更新を使って在庫を設定してください。",
    whatnotIntro:
      "Whatnotの一括出品ファイルです。Seller Hub（Inventory → Import from CSV）でインポートすると下書きの出品に、ショー内（Add → Create Temporary Listing → Upload CSV）でインポートすることもできます。鑑定済みスラブはGradedとして出品され、鑑定会社とグレードがタイトルと説明に入ります。価格はドル単位で切り上げます。",
    type: "種類",
    listingType: "出品形式",
    shippingProfile: "配送プロファイル",
    file: "ファイル",
    shopifyFile: "Shopifyファイル",
    products: "商品",
    condition: "グレードなしカードのコンディション",
    conditionNote: "Ximilarでコンディションを判定したカードは、その判定結果を使います。",
    location: "ロケーション",
    locationPlaceholder: "例：店舗",
    locationNote: "Shopifyでの名前と完全に一致させてください（設定 → ロケーション）。",
    uploadPictures: "画像をアップロード（ImgBB）",
    publishActive: "下書きではなく公開（アクティブ）にする",
    offerable: "購入者が価格を提案できる",
    noImgbbKey:
      "{name}は画像をリンクからダウンロードします。アップロードするには、設定でImgBB APIキーを追加してください。キーがない場合は{name}で写真を追加してください。",
    loadingCards: "カードを読み込み中…",
    noCards: "このセッションには識別済みのカードがまだありません。",
    variantNote: "同じコンディションのカードは1つのバリエーションにまとめられ、そのうち最も高い価格が使われます。",
  },
  result: {
    title: "{name}用にエクスポートしました",
    showFile: "ファイルを表示",
    done: "完了",
    products: { other: "{count}件の商品" },
    variants: { other: "{count}件のバリエーション" },
    productsVariants: "{products}（{variants}）",
    saved: "{what}を<path></path>に保存しました。",
    withoutPictures: {
      other: "{count}件は画像がありません：公開する前に{name}で追加してください。",
    },
    leftOut: "除外されたもの：",
  },
};
export default messages;
