import type { Messages } from "..";
import type en from "../en/models";

const messages: Messages<typeof en> = {
  picker: {
    model: "Modell",
    downloaded: "Heruntergeladen",
  },
  card: {
    sizes: "{size} Download · ~{memory} Speicher",
    modelPage: "Modellseite",
    unsupported: "Dieser Build von Cardloader wurde ohne Unterstützung für lokale Modelle erstellt.",
    progress: "{done} von {size} ({percent} %)",
    left: "noch ~{time}",
    ready: "Heruntergeladen · läuft auf diesem Computer, ohne Internet",
    accept: "Ich akzeptiere die <license>{license}</license>",
    licenseNote:
      "Das Modell stammt von Liquid AI und wird von Hugging Face geladen; es ist nicht Teil von Cardloader. Seine Lizenz erlaubt kommerzielle Nutzung nur unterhalb einer Umsatzgrenze — lies sie, bevor du sie akzeptierst.",
    resume: "Download fortsetzen",
    download: "Herunterladen",
    partial: "{done} von {size} bereits heruntergeladen",
  },
  compare: {
    title: "Mit einem lokalen Modell vergleichen",
    stop: "Stoppen",
    intro:
      "Ein lokales Modell liest jede Karte, die Ximilar in „{name}“ erkannt hat, und die beiden Antworten werden verglichen. In der Sitzung ändert sich nichts. Probiere mehrere Modelle aus, um zu sehen, welches zu deinen Karten passt.",
    downloadFirst: "Lade zuerst in den Einstellungen ein lokales Modell herunter.",
    model: "Modell",
    compareAgain: "Erneut vergleichen",
    compare: "Vergleichen",
    sameName: "Gleicher Name",
    sameNumber: "Gleiche Nummer",
    perCard: "Pro Karte",
    loadingModel: "Modell wird geladen…",
    progress: { one: "{done} von {count} Karte", other: "{done} von {count} Karten" },
    of: "{n} von {total}",
    ofWithNumber: "{n} von {total} mit Nummer",
    onThisComputer: "auf diesem Computer",
    localModel: "Lokales Modell",
    name: "Name",
    number: "Nr.",
    nothingRead: "nichts gelesen",
    noCards: "Diese Sitzung hat keine von Ximilar erkannten Karten zum Vergleichen.",
    same: "gleich",
    different: "abweichend",
  },
};
export default messages;
