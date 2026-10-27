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
    pricesNeedApi: "Preise brauchen die Ximilar API.",
    roughGrades:
      "Bewertungen und Zustände des lokalen Modells sind grobe Schätzungen eines kleinen Modells, nicht die eines Graders.",
  },
};
export default messages;
