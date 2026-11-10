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
      "Das Modell stammt von {maker} und wird von Hugging Face geladen; es ist nicht Teil von Cardloader. Lies seine Lizenz, bevor du sie akzeptierst.",
    licenseLimit: "Sie erlaubt kommerzielle Nutzung nur unterhalb einer Umsatzgrenze.",
    resume: "Download fortsetzen",
    download: "Herunterladen",
    partial: "{done} von {size} bereits heruntergeladen",
  },
  device: {
    runOn: "Ausführen auf",
    auto: "Automatisch",
    noGpu: "keine GPU gefunden, also die CPU",
    cpu: "Nur CPU",
    integrated: "{backend} · integriert",
    missing: "{name} (nicht gefunden: die CPU wird verwendet)",
    tooSmall:
      "{model} braucht etwa {need}, {gpu} hat {have}: Was nicht hineinpasst, läuft auf der CPU und ist langsamer.",
    loaded: "Das geladene Modell läuft auf {runsOn}.",
    partly:
      "Das geladene Modell läuft mit {gpu} von {layers} Schichten auf {runsOn} und mit dem Rest auf der CPU: Der freie GPU-Speicher reichte nicht für alles.",
    fellBack: "Die GPU konnte das Modell nicht laden, daher läuft es auf der CPU ({error}).",
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
