import type { Messages } from "..";
import type en from "../en/newSession";

const messages: Messages<typeof en> = {
  title: {
    new: "Neue Sitzung",
    edit: "Sitzungseinstellungen",
    duplicate: "Sitzung duplizieren",
  },
  copyName: "{name} (Kopie)",
  importing: "Wird importiert…",
  import: "Importieren",
  create: "Erstellen",
  importAndRun: "Importieren & starten",
  importAndRunTitle: "Importieren, dann die Bilder analysieren und wie unten eingestellt hochladen",
  createAndRun: "Erstellen & starten",
  needsToken: "Hinterlege deinen Ximilar-API-Token in den Einstellungen, bevor du eine Sitzung startest.",
  needsModel: "Lade in den Einstellungen ein lokales Modell herunter, bevor du diese Sitzung startest.",
  editNote:
    "Änderungen gelten für die ab jetzt analysierten Karten. Bereits analysierte Karten behalten ihre Ergebnisse; mit <b>Duplizieren</b> analysierst du dieselbe Quelle erneut mit anderen Einstellungen.",
  name: "Name",
  source: "Quelle",
  sources: {
    folder: "Die Bilder in einem Ordner analysieren",
    watch_folder: "Bilder analysieren, sobald eine andere App sie speichert",
    scanner: "Einen Kartenstapel scannen (z. B. fi-8170)",
    csv: "Karten oder Bildpfade aus einer Tabelle importieren",
  },
  browse: "Durchsuchen",
  csv: {
    label: "CSV-Datei",
    help: "Eine Karte pro Zeile. Zeilen mit Kartennamen (und Set, Nummer, Preis, Bewertung…) werden als erkannte Karten importiert; Zeilen mit nur einem Bildpfad werden analysiert. Ein CSV-Export von Cardloader wird wieder so importiert, wie er war.",
    reanalyze: "Die Bilder der {count} erkannten Zeilen erneut analysieren",
    summary: "<b>{rows}</b>: {cards} zum Import, {images} zur Analyse.",
    summarySkipped: "<b>{rows}</b>: {cards} zum Import, {images} zur Analyse, {skipped} leere übersprungen.",
    rows: { one: "{count} Zeile", other: "{count} Zeilen" },
    identifiedCards: { one: "{count} erkannte Karte", other: "{count} erkannte Karten" },
    images: { one: "{count} Bild", other: "{count} Bilder" },
    missingImages: {
      one: "{count} Zeile verweist auf fehlende Bilder.",
      other: "{count} Zeilen verweisen auf fehlende Bilder.",
    },
    columns: "Spalten: {columns}",
  },
  folder: {
    label: "Ordner",
    chooseWatch: "Wähle den Ordner, in den deine Scan-App Bilder speichert.",
    choose: "Wähle den Ordner mit deinen Kartenbildern.",
    noImages: "Keine Bilder in diesem Ordner (JPEG, PNG, WebP, TIFF oder BMP).",
    found: { one: "{count} Bild gefunden", other: "{count} Bilder gefunden" },
    foundWatching: {
      one: "{count} Bild gefunden — neue Bilder werden übernommen, solange die Sitzung läuft",
      other: "{count} Bilder gefunden — neue Bilder werden übernommen, solange die Sitzung läuft",
    },
  },
  cost: {
    local: "auf diesem Computer erkannt, keine API-Aufrufe",
    calls: { one: "etwa {count} API-Aufruf", other: "etwa {count} API-Aufrufe" },
    callsCloud: {
      one: "etwa {count} API-Aufruf für Cards Cloud",
      other: "etwa {count} API-Aufrufe für Cards Cloud",
    },
    callsMagicAi: {
      one: "etwa {count} API-Aufruf (Magic AI kann mehr verursachen)",
      other: "etwa {count} API-Aufrufe (Magic AI kann mehr verursachen)",
    },
    localGrade: "auf diesem Computer gegradet, keine API-Aufrufe",
    none: "keine API-Aufrufe",
    agent: "von deinem KI-Agenten erkannt, keine API-Aufrufe",
  },
  scanner: {
    label: "Scanner",
    searching: "Suche läuft…",
    noneFound: "Kein Scanner gefunden",
    resolution: "Auflösung",
    dpiWhenNotGrading: "300 dpi ohne Grading.",
    sides: "Seiten",
    duplex: "Vorder- + Rückseite",
    frontOnly: "Nur Vorderseite",
    multifeed: "Doppeleinzugserkennung",
    shared: "Das sind deine Scanner-Einstellungen: Eine Änderung hier gilt für jede Scanner-Sitzung.",
  },
  identifyWith: "Erkennen mit",
  model: "Modell",
  noModel: "Kein Modell heruntergeladen",
  notDownloaded: "nicht heruntergeladen",
  localNote:
    "Läuft auf diesem Computer: liest Name, Sammlernummer und Jahr, die auf jeder Karte gedruckt sind, und schätzt Bewertung und Zustand. Keine Preise oder weiteren Kandidaten.",
  localBack: {
    label: "Vorder- und Rückseite zur Identifizierung verwenden",
    hint: "Paart Vorder- und Rückseite jeder Karte und zeigt dem Modell beide: Sportkarten tragen Kartennummer, Jahr und Set auf der Rückseite. Bei Sportkarten standardmäßig an. Etwa doppelt so lange pro Karte.",
  },
  identifyAs: "Erkennen als",
  grading: "Grading",
  imagesPerCard: "Bilder pro Karte",
  pairingWarning: {
    single: {
      title: "Jedes Bild wird eine eigene Karte.",
      text: "Enthalten die Bilder auch die Rückseiten, wird jede Rückseite als weitere Karte analysiert, mit eigenen API-Aufrufen und eigenem Preis. Wähle dann „Paare aus Vorder- + Rückseite“.",
    },
    frontBack: {
      title: "Je zwei Bilder werden eine Karte.",
      text: "Vorder- und Rückseiten werden nach Namen gepaart (card_front / card_back, card_F / card_B), sonst der Reihe nach je zwei: vorn, hinten, vorn, hinten. Sind es nur Vorderseiten, würde jede zweite Karte als Rückseite der vorigen genommen und fehlen.",
    },
    odd: "Dieser Ordner hat eine ungerade Zahl von Bildern ({count}): Eine Karte bliebe ohne Rückseite, oder die Paare sind verschoben. Prüfe, ob eine Seite fehlt.",
  },
  conditionScale: "Zustandsskala",
  priceStats: "Preisstatistik",
  readSlabs: "Etiketten bewerteter Slabs lesen",
  syncRemote: "Mit Ximilar Cards Cloud synchronisieren",
  known: {
    title: "Schneller und genauer",
    intro:
      "Sag Ximilar, was du über die Karten schon weißt, damit es das nicht erst ermitteln muss. Hake nur an, was für jedes Bild stimmt: Ein falscher Wert verschlechtert die Treffer.",
    skipDetection: "Detektion überspringen",
    skipDetectionHint:
      "Für Bilder, die schon auf die Karte zugeschnitten sind: Jedes Bild wird als die Karte selbst gesendet, sodass Ximilar sie nicht im Bild sucht.",
    skipDetectionSlab: "Slab-Etiketten werden per Detektion gefunden und dann evtl. nicht gelesen.",
    everyCard: {
      tcg: "Jede Karte ist eine Sammelkarte",
      sport: "Jede Karte ist eine Sportkarte",
      comics: "Jede Karte ist ein Comic",
      slab: "Jede Karte ist ein bewerteter Slab",
    },
    cardType: "Kartentyp",
    sendsCategory: "Sendet Kategorie „{category}“.",
    chooseTypeFirst: "Wähle zuerst einen Typ unter „Erkennen als“; Automatisch überlässt es Ximilar.",
    frontSide: "Vorderseite",
    frontSideHint:
      "Die Bilder zeigen die Vorderseite (mit „Vorder- und Rückseite“ gesendete Rückseiten werden als Rückseite markiert).",
    upright: "Aufrecht",
    uprightHint:
      "Keine Karte ist gedreht oder steht auf dem Kopf. Sonst richtet Ximilar jede Karte vor der Erkennung auf.",
    game: "Spiel",
    alphabet: "Schrift",
    gameNote:
      "Das Spiel grenzt die Suche auf seine Karten ein; die Schrift (z. B. Japanisch für japanische Drucke) verhindert Treffer mit der Version einer anderen Sprache.",
    gameNoAlphabet:
      "Mit einem gewählten Spiel liest Ximilar die Schrift nicht mehr und nimmt Lateinisch an: Wähle sie für Drucke in anderen Schriften.",
    sport: "Sportart",
    sportNote: "Grenzt die Suche auf die Karten dieser Sportart ein.",
  },
  subcategory: {
    anyGame: "Jedes Spiel",
    anySport: "Jede Sportart",
    readFromCard: "Ximilar liest es von der Karte",
  },
  alphabets: {
    detect: "Erkennen",
    latin: "Lateinisch",
    latinHint: "Englisch, europäische Sprachen",
    japanese: "Japanisch",
    chinese: "Chinesisch",
    korean: "Koreanisch",
    russian: "Russisch",
    thai: "Thai",
  },
  more: {
    label: "Weitere Optionen",
    summary: "— was du über die Karten weißt, Vorder- und Rückseite, Magic AI, eigener Endpunkt",
  },
  bothSides: {
    label: "Anhand von Vorder- und Rückseite erkennen",
    csv: "Nutzt die Rückseiten-Bildspalte der CSV.",
    paired:
      "Bilder namens <code>card_F.jpg</code> und <code>card_B.jpg</code> (oder front/back) werden nach Namen gepaart, sonst aufeinanderfolgende Bilder.",
    why: "Die Rückseite geht mit ihrer Vorderseite in die Erkennung — das hilft bei Sportkarten, deren Rückseiten Spieler, Jahr und Nummer zeigen.",
    frontOnly: "„{type}“ liest nur die Vorderseite: Die Rückseiten sehen alle gleich aus.",
    magicAi: "Magic AI für Sportkarten",
    magicAiHint:
      "Ist eine Sportkarte nicht in Ximilars Datenbank, erkennt ein KI-Modell sie stattdessen anhand ihrer Bilder. Zusätzliche API-Credits fallen nur an, wenn es läuft.",
    magicAiSportOnly: "Gilt nur für Sportkarten.",
    autoNote:
      "Automatisch erkennt eine Karte mit einem Aufruf; eine Sportkarte wird für diese Optionen dann zusätzlich an Sports ID gesendet (ein weiterer Aufruf).",
    customEndpoint: "Eigener Endpunkt (optional)",
    customEndpointHint:
      "Ein Erkennungs-Endpunkt, den Ximilar für dich eingerichtet hat. Er bekommt dieselbe Anfrage wie der Standard-Endpunkt (Vorder- und Rückseite, wenn oben gewählt), und seine Antworten werden als „{type}“ gelesen.",
    customEndpointHintAuto:
      "Ein Erkennungs-Endpunkt, den Ximilar für dich eingerichtet hat. Er bekommt dieselbe Anfrage wie der Standard-Endpunkt (Vorder- und Rückseite, wenn oben gewählt), und seine Antworten werden als der erkannte Kartentyp gelesen.",
  },
  options: {
    dpiIdentification: "Erkennung",
    dpiGrading: "Grading",
    tradingCards: "Sammelkarten",
    sportsCards: "Sportkarten",
    apiOnly: "nur API",
    multifeedOffHint: "empfohlen für Karten",
    multifeedDefault: "Treiber-Standard",
    single: "Ein Bild pro Karte",
    frontBack: "Paare aus Vorder- + Rückseite",
  },
  openSettings: "Einstellungen öffnen",
  remote: {
    label: "Eine Cards-Cloud-Sitzung fortsetzen (optional)",
    placeholder: "Sitzungs-ID oder Link",
    untitled: "Ohne Titel",
    found: { one: "„{name}“ · {count} Karte", other: "„{name}“ · {count} Karten" },
    paste: "Füge die ID der Sitzung oder ihren Link aus der Ximilar App ein.",
    added: "{found}. Neue Karten werden ihr hinzugefügt.",
    addedSkips:
      "{found}. Neue Karten werden ihr hinzugefügt; Bilder, die sie schon hat (nach Dateiname), werden übersprungen.",
    lookingUp: "Wird gesucht…",
    hint: "Fügt die Karten einer früher begonnenen Sitzung hinzu, z. B. auf einem anderen Computer.",
    hintSkips:
      "Fügt die Karten einer früher begonnenen Sitzung hinzu, z. B. auf einem anderen Computer; Bilder, die sie schon hat (nach Dateiname), werden übersprungen.",
  },
  task: "Aufgabe",
  gradeWith: "Graden mit",
  cropWith: "Zuschneiden mit",
  crop: {
    folder: "Zuschnitte speichern in",
    choose: "Wähle den Ordner, in den die zugeschnittenen Karten kommen.",
    hint: "Dateien mit den Namen der Kartenbilder; ein vergebener Name bekommt _2, _3…",
    level: "Farben ausgleichen und schärfen",
    levelHint: "Aus: die Karte wie gescannt, nur ausgeschnitten und gerade gerückt.",
    also: "Zugeschnittene Karten auch speichern",
    alsoHint:
      "Jede Karte aus ihrem Bild ausgeschnitten und gerade gerückt, als Bilder in einem Ordner. Ein API-Aufruf mehr pro Karte.",
    format: "Dateityp",
    formats: {
      jpg: "Klein, öffnet sich überall",
      png: "Verlustfrei, größere Dateien",
      webp: "Wie Ximilar es schickt: am kleinsten",
      webpCut: "Verlustfrei: scharf, größere Dateien",
    },
  },
  needsAgents:
    "Schalte in den Einstellungen KI-Agenten mit „Starten und prüfen“ ein, damit dein Agent die Karten erkennen kann.",
  agentNote:
    "Dein KI-Agent liest jede Karte anhand ihrer Bilder: Bitte ihn, z. B. in Claude, die Karten dieser Sitzung in Cardloader zu erkennen.",
  agentHint: "Claude oder ein anderer per MCP verbundener Agent",
  whatToDo: "Was tun",
  tasks: { identify: "Erkennen", prices: "Preise", grade: "Graden", crop: "Zuschneiden" },
  taskHints: {
    identify: "Name, Set und Nummer",
    prices: "Wofür jede Karte verkauft wird",
    grade: "Eine Note oder der Zustand",
    crop: "Ausgeschnitten und gerade gerückt",
  },
  needsIdentify: "Braucht Erkennen",
  keepOne: "Eine Sitzung macht mindestens eines davon.",
  identifyLocked: "Diese Sitzung hat Karten: Sie erkennt sie weiter, oder weiter nicht.",
  priceWith: "Preise mit",
  priceEngines: {
    ximilar: "Ximilar-Preisstatistik",
    ebay: "eBay-Angebote",
    agent: "KI-Agent (MCP)",
    ebay_local: "eBay-Angebote + lokales Modell",
    ebay_agent: "eBay-Angebote + KI-Agent (MCP)",
  },
  priceHints: {
    ximilar: "Kommt mit der Erkennung von Ximilar",
    ebay: "Der Median der aktiven Angebote der Karte",
    agent: "Claude oder ein anderer per MCP verbundener Agent",
    ebay_local: "Das lokale Modell behält die Angebote, die die Karte sind",
    ebay_agent: "Dein Agent prüft die Angebote und ihre Bilder",
  },
  priceNotes: {
    ximilar: "Ximilars Marktpreise kommen mit jeder Erkennung, ohne Zusatzkosten.",
    ebay: "Sobald eine Karte erkannt ist, sucht Cardloader mit deinem eBay-Keyset ihre aktiven eBay-Angebote zum Festpreis und nimmt deren Median als Preis. Die Angebote bleiben an der Karte.",
    agent:
      "Dein KI-Agent bepreist jede Karte, sobald sie erkannt ist: Bitte ihn, z. B. in Claude, die Karten dieser Sitzung in Cardloader zu bepreisen.",
    ebay_local:
      "Sobald eine Karte erkannt ist, sucht Cardloader mit deinem eBay-Keyset ihre aktiven eBay-Angebote zum Festpreis. Das lokale Modell liest die Titel und behält die Angebote, die diese Karte sind (dieselbe Karte, Variante und Bewertung; keine Konvolute), und deren Median wird ihr Preis. Die Angebote bleiben an der Karte, die aussortierten ausgegraut.",
    ebay_agent:
      "Sobald eine Karte erkannt ist, sucht Cardloader mit deinem eBay-Keyset ihre aktiven eBay-Angebote zum Festpreis und übergibt sie deinem KI-Agenten, mit ihren Bildern und denen der Karte. Der Agent behält die Angebote, die diese Karte sind, und deren Median wird ihr Preis: Bitte ihn, z. B. in Claude, die Karten dieser Sitzung in Cardloader zu bepreisen.",
  },
  needsEbay:
    "Füge in den Einstellungen dein eBay-Keyset hinzu, damit Cardloader eBay-Angebote nach Preisen durchsuchen kann.",
  needsAgentsPrices:
    "Schalte in den Einstellungen KI-Agenten mit „Starten und prüfen“ ein, damit dein Agent die Karten bepreisen kann.",
  needsAgentsCrop:
    "Schalte in den Einstellungen KI-Agenten mit „Starten und prüfen“ ein, damit dein Agent die Karten zuschneiden kann.",
  gradeWhat: "Was graden",
  sameModel: "Dasselbe Modell erkennt die Karten.",
  cropNotes: {
    ximilar: "Ximilar schneidet jede Karte aus ihrem Scan aus und rückt sie gerade: ein weiterer API-Aufruf pro Karte.",
    local:
      "Das Modell auf diesem Computer findet die Ecken jeder Karte, Cardloader schneidet sie aus und rückt sie gerade. Keine API-Aufrufe; wie gut der Zuschnitt sitzt, hängt vom Modell ab.",
    agent:
      "Dein KI-Agent findet die Ecken jeder Karte, Cardloader schneidet sie aus und rückt sie gerade: Bitte ihn, z. B. in Claude, die Karten dieser Sitzung in Cardloader zuzuschneiden.",
  },
  missing: {
    csv: "Wähle die CSV-Datei zum Importieren.",
    csvUnreadable: "Die CSV-Datei kann nicht gelesen werden.",
  },
};
export default messages;
