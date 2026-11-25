import type { Messages } from "..";
import type en from "../en/wizard";

const messages: Messages<typeof en> = {
  title: "Cardloader einrichten",
  progress: "Schritt {step} von {steps} · du kannst alles später in den Einstellungen ändern",
  alwaysIncluded: "Immer dabei",
  language: {
    title: "Wähle deine Sprache",
    text: "Cardloader spricht ab jetzt diese Sprache.",
  },
  interests: {
    title: "Was wirst du scannen?",
    text: "Wähle alles, was du sammelst.",
    tcg: "Sammelkartenspiele",
    tcgHint: "Pokémon, Magic, Yu-Gi-Oh!, One Piece, Lorcana…",
    sport: "Sportkarten",
    sportHint: "Baseball, Basketball, Football, Eishockey, Fußball…",
    comics: "Comics",
    comicsHint: "Comichefte, Magazine und Manga",
  },
  games: {
    title: "Welche Spiele?",
    text: "Wählst du eines, ist jede neue Sitzung darauf eingestellt und Ximilar sucht nur in diesem Spiel. Wähle mehrere oder keines, um es pro Sitzung festzulegen.",
  },
  goals: {
    title: "Was möchtest du herausfinden?",
    text: "Wähle alles, was du brauchst.",
    identification: "Erkennung",
    identificationHint: "Name, Set und Nummer jeder Karte",
    grading: "Grading",
    gradingHint: "Eine volle Bewertung: Ecken, Kanten, Oberfläche und Zentrierung",
    condition: "Zustand",
    conditionHint: "Ein schneller Zustand, z. B. Near Mint, für Angebote",
    prices: "Preise",
    pricesHint: "Marktpreise jeder Karte",
    crops: "Zugeschnittene Karten",
    cropsHint: "Jede Karte ausgeschnitten und gerade gerückt, in einem Ordner gespeichert",
  },
  sources: {
    title: "Wie kommen die Karten herein?",
    text: "Wähle alles, was du nutzt. Die erste Quelle öffnet sich bei „Neue Sitzung“.",
    scanner: "Scanner",
    scannerHint: "Ein Dokumentenscanner mit Einzug, z. B. RICOH fi-8170",
    folder: "Bilderordner",
    folderHint: "Fotos oder Scans, die schon auf diesem Computer sind",
    watchFolder: "Überwachter Ordner",
    watchFolderHint: "Karten werden analysiert, sobald neue Bilder in einem Ordner landen",
    csv: "CSV-Datei",
    csvHint: "Eine Liste von Karten oder Bildpfaden",
  },
  engine: {
    title: "Was soll die Karten erkennen?",
    ximilar: "Ximilar API",
    ximilarHint: "Genaue Erkennung, Preise und Grading. Braucht einen API-Token und Internet.",
    local: "Lokales KI-Modell",
    localHint:
      "Läuft auf diesem Computer, offline und kostenlos. Liest Name, Nummer und Set und schätzt den Zustand; weniger genau.",
    token: "API-Token",
    tokenSaved: "Schon gespeichert, leer lassen, um ihn zu behalten",
    tokenPaste: "Token einfügen",
    tokenHelp:
      "Du findest ihn in der <link>Ximilar App</link> unter deinem Profil. Du kannst ihn später in den Einstellungen hinzufügen.",
    pricesNeedApi:
      "Ximilars Preise gibt es nur mit seiner Erkennung: Mit dem lokalen Modell nimm die Preise aus eBay-Angeboten (deine eBay-Schlüssel in den Einstellungen).",
    roughGrades:
      "Bewertungen und Zustände des lokalen Modells sind grobe Schätzungen eines kleinen Modells, nicht die eines Graders.",
    cropsNeedApi:
      "Zugeschnittene Bilder macht die Ximilar-API (füge dein Token in den Einstellungen hinzu), außer du wählst das lokale Modell im Tab Zuschneiden von Neue Sitzung.",
    gradeTitle: "Was soll die Karten graden?",
  },
  task: {
    title: "Was soll Cardloader mit deinen Karten tun?",
    text: "Neue Sitzungen beginnen damit; du kannst für jede Sitzung neu wählen.",
    identify: "Erkennen",
    identifyHint: "Name, Set, Nummer und Preis jeder Karte, auf Wunsch mit Note",
    grade: "Nur graden",
    gradeHint: "Eine volle Note jeder Karte (Ecken, Kanten, Oberfläche, Zentrierung), ohne sie zu erkennen",
    condition: "Nur den Zustand",
    conditionHint: "Der Zustand jeder Karte, z. B. Near Mint, für Angebote, ohne sie zu erkennen",
    crop: "Nur zuschneiden",
    cropHint: "Jede Karte aus ihrem Bild ausgeschnitten und gerade gerückt, als Bild in einem Ordner gespeichert",
  },
  token: {
    title: "Ximilar-API verbinden",
    text: "Das Zuschneiden läuft über die Ximilar-API: Es braucht ein API-Token und Internet.",
  },
};
export default messages;
