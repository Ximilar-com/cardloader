import type { Messages } from "..";
import type en from "../en/newSession";

const messages: Messages<typeof en> = {
  title: {
    new: "新しいセッション",
    edit: "セッションの設定",
    duplicate: "セッションを複製",
  },
  copyName: "{name}（コピー）",
  importing: "インポート中…",
  import: "インポート",
  create: "作成",
  importAndRun: "インポートして実行",
  importAndRunTitle: "インポートしてから、下の設定どおりに画像を解析・アップロードします",
  createAndRun: "作成して実行",
  needsToken: "セッションを実行する前に、設定でXimilar APIトークンを入力してください。",
  needsModel: "このセッションを実行する前に、設定でローカルモデルをダウンロードしてください。",
  editNote:
    "変更はこれ以降に解析するカードに適用されます。解析済みのカードの結果はそのままです。同じ取り込み元を別の設定で解析し直すには<b>複製</b>を使ってください。",
  name: "名前",
  source: "取り込み元",
  sources: {
    folder: "フォルダー内の画像を解析",
    watch_folder: "ほかのアプリが保存した画像を順次解析",
    scanner: "カードの束をスキャン（例：fi-8170）",
    csv: "スプレッドシートからカードや画像パスをインポート",
  },
  browse: "参照",
  csv: {
    label: "CSVファイル",
    help: "1行に1枚のカードです。カード名（とセット、番号、価格、グレードなど）がある行は識別済みのカードとしてインポートされ、画像パスだけの行は解析されます。CardloaderのCSVエクスポートはそのままの状態でインポートし直せます。",
    reanalyze: "識別済みの{count}行の画像を再解析する",
    summary: "<b>{rows}</b>：インポート {cards}、解析 {images}。",
    summarySkipped: "<b>{rows}</b>：インポート {cards}、解析 {images}、空行{skipped}件をスキップ。",
    rows: { other: "{count}行" },
    identifiedCards: { other: "識別済みカード{count}枚" },
    images: { other: "画像{count}枚" },
    missingImages: { other: "{count}行が存在しない画像を指しています。" },
    columns: "列：{columns}",
  },
  folder: {
    label: "フォルダー",
    chooseWatch: "スキャンアプリが画像を保存するフォルダーを選択してください。",
    choose: "カード画像のあるフォルダーを選択してください。",
    noImages: "このフォルダーには画像（JPEG、PNG、WebP、TIFF、BMP）がありません。",
    found: { other: "{count}枚の画像が見つかりました" },
    foundWatching: {
      other: "{count}枚の画像が見つかりました — セッションの実行中は新しい画像も取り込まれます",
    },
  },
  cost: {
    local: "このパソコン上で識別、API呼び出しなし",
    calls: { other: "API呼び出し約{count}回" },
    callsCloud: { other: "Cards Cloud用にAPI呼び出し約{count}回" },
    callsMagicAi: {
      other: "API呼び出し約{count}回（Magic AIで増える場合があります）",
    },
  },
  scanner: {
    label: "スキャナー",
    searching: "検索中…",
    noneFound: "スキャナーが見つかりません",
    resolution: "解像度",
    dpiWhenNotGrading: "グレーディングしない場合は300 dpiです。",
    sides: "読み取り面",
    duplex: "表面＋裏面",
    frontOnly: "表面のみ",
    multifeed: "重送検知",
    shared: "これはスキャナーの共通設定です。ここで変更すると、すべてのスキャナーセッションに反映されます。",
  },
  identifyWith: "識別方法",
  model: "モデル",
  noModel: "ダウンロード済みのモデルがありません",
  notDownloaded: "未ダウンロード",
  localNote:
    "このパソコン上で動作します。各カードに印刷された名前、コレクター番号、年を読み取り、グレードとコンディションを推定します。価格やほかの候補は表示されません。",
  identifyAs: "識別の種類",
  grading: "グレーディング",
  imagesPerCard: "カードあたりの画像",
  conditionScale: "コンディションの基準",
  priceStats: "価格の統計",
  readSlabs: "鑑定済みスラブのラベルを読み取る",
  syncRemote: "Ximilar Cards Cloudに同期",
  known: {
    title: "より速く、より正確に",
    intro:
      "カードについてわかっていることをXimilarに伝えると、その判定を省略できます。すべての画像に当てはまるものだけをチェックしてください。誤った値は一致の精度を下げます。",
    skipDetection: "検出をスキップ",
    skipDetectionHint:
      "カードの形に切り抜き済みの画像向けです。各画像をそのままカードとして送るため、Ximilarは画像内でカードを探しません。",
    skipDetectionSlab: "スラブのラベルは検出で見つけるため、読み取れない場合があります。",
    everyCard: {
      tcg: "すべてトレーディングカード",
      sport: "すべてスポーツカード",
      comics: "すべてコミック",
      slab: "すべて鑑定済みスラブ",
    },
    cardType: "カードの種類",
    sendsCategory: "カテゴリー「{category}」を送信します。",
    chooseTypeFirst: "先に「識別の種類」で種類を選んでください。自動判別ではXimilarに任せます。",
    frontSide: "表面",
    frontSideHint: "画像は表面です（「表面と裏面」で送る裏面は裏面として扱われます）。",
    upright: "正しい向き",
    uprightHint:
      "回転や上下逆のカードはありません。チェックしない場合、Ximilarは各カードを正しい向きにしてから識別します。",
    game: "ゲーム",
    alphabet: "文字",
    gameNote:
      "ゲームを指定すると、そのゲームのカードに絞って検索します。文字（例：日本語版なら日本語）を指定すると、別の言語版との一致を防げます。",
    gameNoAlphabet:
      "ゲームを指定すると、Ximilarは文字を読み取らずラテン文字とみなします。ほかの文字で印刷されたカードでは文字を選んでください。",
    sport: "スポーツ",
    sportNote: "そのスポーツのカードに絞って検索します。",
  },
  subcategory: {
    anyGame: "すべてのゲーム",
    anySport: "すべてのスポーツ",
    readFromCard: "Ximilarがカードから読み取ります",
  },
  alphabets: {
    detect: "自動検出",
    latin: "ラテン文字",
    latinHint: "英語、ヨーロッパの言語",
    japanese: "日本語",
    chinese: "中国語",
    korean: "韓国語",
    russian: "ロシア語",
    thai: "タイ語",
  },
  more: {
    label: "その他のオプション",
    summary: "— カードについてわかっていること、表面と裏面、Magic AI、カスタムエンドポイント",
  },
  bothSides: {
    label: "表面と裏面から識別",
    csv: "CSVの裏面画像の列を使います。",
    paired:
      "<code>card_F.jpg</code>と<code>card_B.jpg</code>（またはfront/back）という名前の画像は名前でペアになり、それ以外は連続する画像がペアになります。",
    why: "裏面は表面と一緒に識別に送られます。選手名、年、番号が裏面にあるスポーツカードで役立ちます。",
    frontOnly: "{type}の識別は表面のみを読み取ります。裏面はどれも同じに見えるためです。",
    magicAi: "スポーツカード向けMagic AI",
    magicAiHint:
      "スポーツカードがXimilarのデータベースにない場合、代わりにAIモデルが画像から識別します。追加のAPIクレジットは実行時にのみ使用されます。",
    magicAiSportOnly: "スポーツカードにのみ適用されます。",
    autoNote:
      "自動判別は1回の呼び出しでカードを識別します。これらのオプションでは、スポーツカードはさらにSports IDにも送られます（呼び出し1回追加）。",
    customEndpoint: "カスタムエンドポイント（任意）",
    customEndpointHint:
      "Ximilarがあなた用に設定した識別エンドポイントです。標準のものと同じリクエスト（上で選んだ場合は表面と裏面）を受け取り、その回答は「{type}」として読み取られます。",
    customEndpointHintAuto:
      "Ximilarがあなた用に設定した識別エンドポイントです。標準のものと同じリクエスト（上で選んだ場合は表面と裏面）を受け取り、その回答は検出されたカードの種類として読み取られます。",
  },
  options: {
    dpiIdentification: "識別",
    dpiGrading: "グレーディング",
    tradingCards: "トレーディングカード",
    sportsCards: "スポーツカード",
    apiOnly: "APIのみ",
    multifeedOffHint: "カードにおすすめ",
    multifeedDefault: "ドライバーの既定値",
    single: "1枚につき画像1つ",
    frontBack: "表面＋裏面のペア",
  },
  openSettings: "設定を開く",
  remote: {
    label: "Cards Cloudのセッションを続ける（任意）",
    placeholder: "セッションIDまたはリンク",
    untitled: "無題",
    found: { other: "「{name}」· {count}枚" },
    paste: "Ximilar AppからセッションのIDまたはリンクを貼り付けてください。",
    added: "{found}。新しいカードはこのセッションに追加されます。",
    addedSkips:
      "{found}。新しいカードはこのセッションに追加されます。既にある画像（ファイル名で照合）はスキップされます。",
    lookingUp: "検索中…",
    hint: "以前に（別のパソコンなどで）開始したセッションにカードを追加します。",
    hintSkips:
      "以前に（別のパソコンなどで）開始したセッションにカードを追加します。既にある画像（ファイル名で照合）はスキップされます。",
  },
};
export default messages;
