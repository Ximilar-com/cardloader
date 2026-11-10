import type { Messages } from "..";
import type en from "../en/models";

const messages: Messages<typeof en> = {
  picker: {
    model: "Modello",
    downloaded: "Scaricato",
  },
  card: {
    sizes: "{size} da scaricare · ~{memory} di memoria",
    modelPage: "Pagina del modello",
    unsupported: "Questa build di cardloader è stata creata senza supporto per i modelli locali.",
    progress: "{done} di {size} ({percent}%)",
    left: "~{time} rimanenti",
    ready: "Scaricato · gira su questo computer, senza internet",
    accept: "Accetto la <license>{license}</license>",
    licenseNote:
      "Il modello è di {maker} e viene scaricato da Hugging Face; non fa parte di cardloader. Leggi la sua licenza prima di accettare.",
    licenseLimit: "Consente l’uso commerciale solo sotto una soglia di fatturato.",
    resume: "Riprendi download",
    download: "Scarica",
    partial: "{done} di {size} già scaricati",
  },
  device: {
    runOn: "Esegui su",
    auto: "Automatico",
    noGpu: "nessuna GPU trovata, quindi la CPU",
    cpu: "Solo CPU",
    integrated: "{backend} · integrata",
    missing: "{name} (non trovata: si usa la CPU)",
    tooSmall: "{model} richiede circa {need} e {gpu} ha {have}: la parte che non ci sta gira sulla CPU, più lenta.",
    loaded: "Il modello caricato gira su {runsOn}.",
    partly:
      "Il modello caricato fa girare {gpu} dei suoi {layers} livelli su {runsOn} e il resto sulla CPU: la memoria libera della GPU non bastava.",
    fellBack: "La GPU non è riuscita a caricare il modello, quindi gira sulla CPU ({error}).",
  },
  compare: {
    title: "Confronta con un modello locale",
    stop: "Ferma",
    intro:
      "Un modello locale legge ogni carta che Ximilar ha identificato in “{name}” e le due risposte vengono confrontate. Nella sessione non cambia nulla. Provalo con più modelli per vedere quale va meglio con le tue carte.",
    downloadFirst: "Prima scarica un modello locale in Impostazioni.",
    model: "Modello",
    compareAgain: "Confronta di nuovo",
    compare: "Confronta",
    sameName: "Stesso nome",
    sameNumber: "Stesso numero",
    perCard: "Per carta",
    loadingModel: "Caricamento del modello…",
    progress: { one: "{done} di {count} carta", other: "{done} di {count} carte" },
    of: "{n} di {total}",
    ofWithNumber: "{n} di {total} con un numero",
    onThisComputer: "su questo computer",
    localModel: "Modello locale",
    name: "Nome",
    number: "N.",
    nothingRead: "nessuna lettura",
    noCards: "Questa sessione non ha carte identificate da Ximilar con cui fare il confronto.",
    same: "uguale",
    different: "diverso",
  },
};
export default messages;
