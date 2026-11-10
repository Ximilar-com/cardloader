import type { Messages } from "..";
import type en from "../en/models";

const messages: Messages<typeof en> = {
  picker: {
    model: "Modèle",
    downloaded: "Téléchargé",
  },
  card: {
    sizes: "{size} à télécharger · ~{memory} de mémoire",
    modelPage: "Page du modèle",
    unsupported: "Cette version de cardloader a été compilée sans prise en charge des modèles locaux.",
    progress: "{done} sur {size} ({percent} %)",
    left: "~{time} restant",
    ready: "Téléchargé · fonctionne sur cet ordinateur, sans Internet",
    accept: "J’accepte la <license>{license}</license>",
    licenseNote:
      "Le modèle est créé par {maker} et téléchargé depuis Hugging Face ; il ne fait pas partie de cardloader. Lisez sa licence avant d’accepter.",
    licenseLimit: "Elle n’autorise un usage commercial qu’en dessous d’un seuil de chiffre d’affaires.",
    resume: "Reprendre le téléchargement",
    download: "Télécharger",
    partial: "{done} sur {size} déjà téléchargés",
  },
  device: {
    runOn: "Exécuter sur",
    auto: "Automatique",
    noGpu: "aucun GPU trouvé, donc le CPU",
    cpu: "CPU uniquement",
    integrated: "{backend} · intégré",
    missing: "{name} (introuvable : le CPU est utilisé)",
    tooSmall:
      "{model} a besoin d’environ {need} et {gpu} dispose de {have} : ce qui n’y tient pas tourne sur le CPU, plus lent.",
    loaded: "Le modèle chargé tourne sur {runsOn}.",
    partly:
      "Le modèle chargé fait tourner {gpu} de ses {layers} couches sur {runsOn} et le reste sur le CPU : la mémoire libre du GPU ne suffisait pas.",
    fellBack: "Le GPU n’a pas pu charger le modèle, il tourne donc sur le CPU ({error}).",
  },
  compare: {
    title: "Comparer avec un modèle local",
    stop: "Arrêter",
    intro:
      "Un modèle local lit chaque carte que Ximilar a identifiée dans « {name} », et les deux réponses sont comparées. Rien ne change dans la session. Lancez-le avec plusieurs modèles pour voir lequel convient à vos cartes.",
    downloadFirst: "Téléchargez d’abord un modèle local dans Réglages.",
    model: "Modèle",
    compareAgain: "Comparer à nouveau",
    compare: "Comparer",
    sameName: "Même nom",
    sameNumber: "Même numéro",
    perCard: "Par carte",
    loadingModel: "Chargement du modèle…",
    progress: { one: "{done} sur {count} carte", other: "{done} sur {count} cartes" },
    of: "{n} sur {total}",
    ofWithNumber: "{n} sur {total} avec un numéro",
    onThisComputer: "sur cet ordinateur",
    localModel: "Modèle local",
    name: "Nom",
    number: "N°",
    nothingRead: "rien lu",
    noCards: "Cette session n’a aucune carte identifiée par Ximilar à comparer.",
    same: "identique",
    different: "différent",
  },
};
export default messages;
