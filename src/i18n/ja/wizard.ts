import type { Messages } from "..";
import type en from "../en/wizard";

const messages: Messages<typeof en> = {
  title: "Cardloaderのセットアップ",
  progress: "ステップ {step}/{steps} · あとから「設定」ですべて変更できます",
  alwaysIncluded: "常に含まれます",
  language: {
    title: "言語を選択",
    text: "これ以降、Cardloaderはこの言語で表示されます。",
  },
  interests: {
    title: "何をスキャンしますか？",
    text: "集めているものをすべて選んでください。",
    tcg: "トレーディングカードゲーム",
    tcgHint: "Pokémon、Magic、Yu-Gi-Oh!、One Piece、Lorcana…",
    sport: "スポーツカード",
    sportHint: "野球、バスケットボール、アメフト、ホッケー、サッカー…",
    comics: "コミック",
    comicsHint: "コミックブック、雑誌、マンガ",
  },
  games: {
    title: "どのゲームですか？",
    text: "1つ選ぶと新しいセッションはすべてそのゲームに設定され、Ximilarはそのゲームのカードだけを照合します。複数選ぶか何も選ばなければ、セッションごとに選べます。",
  },
  goals: {
    title: "何を調べたいですか？",
    text: "必要なものをすべて選んでください。",
    identification: "識別",
    identificationHint: "各カードの名前、セット、番号",
    grading: "グレーディング",
    gradingHint: "コーナー、エッジ、表面、センタリングによる本格的なグレード",
    condition: "コンディション",
    conditionHint: "出品用の簡易コンディション（例：Near Mint）",
    prices: "価格",
    pricesHint: "各カードの相場",
    crops: "切り抜いた画像",
    cropsHint: "各カードを切り抜いてまっすぐにし、フォルダーに保存",
  },
  sources: {
    title: "カードをどう取り込みますか？",
    text: "使うものをすべて選んでください。最初のものが「新しいセッション」で開きます。",
    scanner: "スキャナー",
    scannerHint: "フィーダー付きドキュメントスキャナー（例：RICOH fi-8170）",
    folder: "画像フォルダー",
    folderHint: "このパソコンにある写真やスキャン画像",
    watchFolder: "監視フォルダー",
    watchFolderHint: "フォルダーに新しい画像が追加されるたびにカードを解析します",
    csv: "CSVファイル",
    csvHint: "カードまたは画像パスの一覧",
  },
  engine: {
    title: "カードの識別に何を使いますか？",
    ximilar: "Ximilar API",
    ximilarHint: "高精度な識別、価格、グレーディング。APIトークンとインターネット接続が必要です。",
    local: "ローカルAIモデル",
    localHint:
      "このパソコン上でオフライン・無料で動作します。名前、番号、セットを読み取り、コンディションを推定します。精度は劣ります。",
    token: "APIトークン",
    tokenSaved: "保存済みです。そのままにするには空欄のままにしてください",
    tokenPaste: "トークンを貼り付け",
    tokenHelp: "<link>Ximilar App</link>のプロフィールで確認できます。あとから設定で追加することもできます。",
    pricesNeedApi:
      "Ximilarの価格はXimilarの識別にだけ付きます。ローカルモデルでは、eBayの出品から価格を取得してください（「設定」でeBayのキーを登録）。",
    roughGrades:
      "ローカルモデルのグレードとコンディションは小さなモデルによる大まかな推定で、鑑定士の評価ではありません。",
    cropsNeedApi:
      "切り抜き画像はXimilar APIが作成します（「設定」でトークンを追加）。「新しいセッション」の「切り抜き」タブでローカルモデルを選ぶこともできます。",
    gradeTitle: "何でカードをグレーディングしますか？",
  },
  task: {
    title: "カードをどうしますか？",
    text: "新しいセッションはここから始まります。セッションごとに選び直せます。",
    identify: "識別する",
    identifyHint: "各カードの名前、セット、番号、価格。必要ならグレードも",
    grade: "グレーディングだけ",
    gradeHint: "識別せずに、各カードの本格的なグレード（コーナー、エッジ、表面、センタリング）",
    condition: "コンディションだけ",
    conditionHint: "識別せずに、出品用の各カードのコンディション（例：Near Mint）",
    crop: "切り抜くだけ",
    cropHint: "各カードを画像から切り抜いてまっすぐにし、画像としてフォルダーに保存",
  },
  token: {
    title: "Ximilar APIに接続",
    text: "切り抜きはXimilar APIで行います。APIトークンとインターネット接続が必要です。",
  },
};
export default messages;
