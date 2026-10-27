import type { Messages } from "..";
import type en from "../en/newSession";

const messages: Messages<typeof en> = {
  title: {
    new: "Nuova sessione",
    edit: "Impostazioni sessione",
    duplicate: "Duplica sessione",
  },
  copyName: "{name} (copia)",
  importing: "Importazione…",
  import: "Importa",
  create: "Crea",
  importAndRun: "Importa e avvia",
  importAndRunTitle: "Importa, poi analizza le immagini e carica come impostato qui sotto",
  createAndRun: "Crea e avvia",
  needsToken: "Imposta il token API di Ximilar in Impostazioni prima di avviare una sessione.",
  needsModel: "Scarica un modello locale in Impostazioni prima di avviare questa sessione.",
  editNote:
    "Le modifiche valgono per le carte analizzate da ora in poi. Le carte già analizzate mantengono i loro risultati; usa <b>Duplica</b> per analizzare di nuovo la stessa origine con altre impostazioni.",
  name: "Nome",
  source: "Origine",
  sources: {
    folder: "Analizza le immagini di una cartella",
    watch_folder: "Analizza le immagini mentre un’altra app le salva",
    scanner: "Scansiona una pila di carte (ad es. fi-8170)",
    csv: "Importa carte o percorsi di immagini da un foglio di calcolo",
  },
  browse: "Sfoglia",
  csv: {
    label: "File CSV",
    help: "Una carta per riga. Le righe con un nome di carta (e set, numero, prezzo, voto…) vengono importate come carte identificate; le righe con solo un percorso di immagine vengono analizzate. Un’esportazione CSV di cardloader si reimporta così com’era.",
    reanalyze: "Analizza di nuovo le immagini delle {count} righe identificate",
    summary: "<b>{rows}</b>: {cards} da importare, {images} da analizzare.",
    summarySkipped: "<b>{rows}</b>: {cards} da importare, {images} da analizzare, {skipped} vuote saltate.",
    rows: { one: "{count} riga", other: "{count} righe" },
    identifiedCards: { one: "{count} carta identificata", other: "{count} carte identificate" },
    images: { one: "{count} immagine", other: "{count} immagini" },
    missingImages: {
      one: "{count} riga rimanda a immagini mancanti.",
      other: "{count} righe rimandano a immagini mancanti.",
    },
    columns: "Colonne: {columns}",
  },
  folder: {
    label: "Cartella",
    chooseWatch: "Scegli la cartella in cui la tua app di scansione salva le immagini.",
    choose: "Scegli la cartella con le immagini delle tue carte.",
    noImages: "Nessuna immagine in questa cartella (JPEG, PNG, WebP, TIFF o BMP).",
    found: { one: "{count} immagine trovata", other: "{count} immagini trovate" },
    foundWatching: {
      one: "{count} immagine trovata — le nuove immagini vengono prese mentre la sessione è in corso",
      other: "{count} immagini trovate — le nuove immagini vengono prese mentre la sessione è in corso",
    },
  },
  cost: {
    local: "identificate su questo computer, nessuna chiamata API",
    calls: { one: "circa {count} chiamata API", other: "circa {count} chiamate API" },
    callsCloud: {
      one: "circa {count} chiamata API per Cards Cloud",
      other: "circa {count} chiamate API per Cards Cloud",
    },
    callsMagicAi: {
      one: "circa {count} chiamata API (Magic AI può aggiungerne altre)",
      other: "circa {count} chiamate API (Magic AI può aggiungerne altre)",
    },
  },
  scanner: {
    label: "Scanner",
    searching: "Ricerca…",
    noneFound: "Nessuno scanner trovato",
    resolution: "Risoluzione",
    dpiWhenNotGrading: "300 dpi senza gradazione.",
    sides: "Lati",
    duplex: "Fronte + retro",
    frontOnly: "Solo fronte",
    multifeed: "Rilevamento alimentazione multipla",
    shared: "Queste sono le tue impostazioni Scanner: cambiarle qui le cambia per ogni sessione con scanner.",
  },
  identifyWith: "Identifica con",
  model: "Modello",
  noModel: "Nessun modello scaricato",
  notDownloaded: "non scaricato",
  localNote:
    "Gira su questo computer: legge nome, numero di collezione e anno stampati su ogni carta e stima voti e condizione. Niente prezzi né altri candidati.",
  identifyAs: "Identifica come",
  grading: "Gradazione",
  imagesPerCard: "Immagini per carta",
  conditionScale: "Scala delle condizioni",
  priceStats: "Statistiche prezzi",
  readSlabs: "Leggi le etichette degli slab gradati",
  syncRemote: "Sincronizza con Ximilar Cards Cloud",
  known: {
    title: "Più veloce e più preciso",
    intro:
      "Di’ a Ximilar cosa sai già delle carte, così non deve ricavarlo. Spunta solo ciò che vale per ogni immagine: un valore sbagliato peggiora le corrispondenze.",
    skipDetection: "Salta il rilevamento",
    skipDetectionHint:
      "Per immagini già ritagliate sulla carta: ogni immagine viene inviata come la carta stessa, quindi Ximilar non la cerca nella foto.",
    skipDetectionSlab: "Le etichette degli slab si trovano con il rilevamento, quindi potrebbero non essere lette.",
    everyCard: {
      tcg: "Ogni carta è una carta collezionabile",
      sport: "Ogni carta è una carta sportiva",
      comics: "Ogni carta è un fumetto",
      slab: "Ogni carta è uno slab gradato",
    },
    cardType: "Tipo di carta",
    sendsCategory: "Invia Category “{category}”.",
    chooseTypeFirst: "Prima scegli un tipo in “Identifica come”; Rilevamento automatico lo lascia a Ximilar.",
    frontSide: "Lato fronte",
    frontSideHint: "Le immagini mostrano il fronte (i retro inviati con “fronte e retro” vengono segnati come retro).",
    upright: "Dritte",
    uprightHint: "Nessuna carta è ruotata o capovolta. Altrimenti Ximilar raddrizza ogni carta prima di identificarla.",
    game: "Gioco",
    alphabet: "Alfabeto",
    gameNote:
      "Il gioco restringe la ricerca alle sue carte; l’alfabeto (ad es. giapponese per le stampe giapponesi) evita che le carte corrispondano alla versione in un’altra lingua.",
    gameNoAlphabet:
      "Con un gioco impostato, Ximilar non legge più l’alfabeto e assume quello latino: sceglilo per stampe in altre scritture.",
    sport: "Sport",
    sportNote: "Restringe la ricerca alle carte di quello sport.",
  },
  subcategory: {
    anyGame: "Qualsiasi gioco",
    anySport: "Qualsiasi sport",
    readFromCard: "Ximilar lo legge dalla carta",
  },
  alphabets: {
    detect: "Rileva",
    latin: "Latino",
    latinHint: "Inglese, lingue europee",
    japanese: "Giapponese",
    chinese: "Cinese",
    korean: "Coreano",
    russian: "Russo",
    thai: "Thailandese",
  },
  more: {
    label: "Altre opzioni",
    summary: "— cosa sai delle carte, fronte e retro, Magic AI, endpoint personalizzato",
  },
  bothSides: {
    label: "Identifica da fronte e retro",
    csv: "Usa la colonna dell’immagine del retro del CSV.",
    paired:
      "Le immagini chiamate <code>card_F.jpg</code> e <code>card_B.jpg</code> (o front/back) vengono abbinate per nome, altrimenti si usano immagini consecutive.",
    why: "Il retro va al riconoscimento insieme al suo fronte — aiuta con le carte sportive, il cui retro riporta giocatore, anno e numero.",
    frontOnly: "L’identificazione {type} legge solo il fronte: i retro sono tutti uguali.",
    magicAi: "Magic AI per le carte sportive",
    magicAiHint:
      "Quando una carta sportiva non è nel database di Ximilar, un modello IA la identifica dalle sue immagini. I crediti API extra si usano solo quando interviene.",
    magicAiSportOnly: "Vale solo per le carte sportive.",
    autoNote:
      "Il rilevamento automatico identifica una carta con una sola chiamata; per queste opzioni una carta sportiva viene poi inviata anche a Sports ID (una chiamata in più).",
    customEndpoint: "Endpoint personalizzato (facoltativo)",
    customEndpointHint:
      "Un endpoint di riconoscimento che Ximilar ha creato per te. Riceve la stessa richiesta di quello standard (fronte e retro se scelti sopra) e le sue risposte vengono lette come “{type}”.",
    customEndpointHintAuto:
      "Un endpoint di riconoscimento che Ximilar ha creato per te. Riceve la stessa richiesta di quello standard (fronte e retro se scelti sopra) e le sue risposte vengono lette come il tipo di carta rilevato.",
  },
  options: {
    dpiIdentification: "identificazione",
    dpiGrading: "gradazione",
    tradingCards: "carte collezionabili",
    sportsCards: "carte sportive",
    apiOnly: "solo API",
    multifeedOffHint: "consigliato per le carte",
    multifeedDefault: "Predefinito del driver",
    single: "Un’immagine per carta",
    frontBack: "Coppie fronte + retro",
  },
  openSettings: "Apri Impostazioni",
  remote: {
    label: "Continua una sessione di Cards Cloud (facoltativo)",
    placeholder: "ID o link della sessione",
    untitled: "Senza titolo",
    found: { one: "“{name}” · {count} carta", other: "“{name}” · {count} carte" },
    paste: "Incolla l’ID della sessione o il suo link dalla Ximilar App.",
    added: "{found}. Le nuove carte vengono aggiunte a questa sessione.",
    addedSkips:
      "{found}. Le nuove carte vengono aggiunte a questa sessione; le immagini che ha già (riconosciute dal nome del file) vengono saltate.",
    lookingUp: "Ricerca…",
    hint: "Aggiunge le carte a una sessione iniziata prima, ad es. su un altro computer.",
    hintSkips:
      "Aggiunge le carte a una sessione iniziata prima, ad es. su un altro computer; le immagini che ha già (riconosciute dal nome del file) vengono saltate.",
  },
};
export default messages;
