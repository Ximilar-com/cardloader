import type { Messages } from "..";
import type en from "../en/wizard";

const messages: Messages<typeof en> = {
  title: "Configurer Cardloader",
  progress: "Étape {step} sur {steps} · vous pourrez tout modifier plus tard dans Réglages",
  alwaysIncluded: "Toujours inclus",
  language: {
    title: "Choisissez votre langue",
    text: "Cardloader l’utilisera désormais.",
  },
  interests: {
    title: "Qu’allez-vous numériser ?",
    text: "Cochez tout ce que vous collectionnez.",
    tcg: "Jeux de cartes à collectionner",
    tcgHint: "Pokémon, Magic, Yu-Gi-Oh!, One Piece, Lorcana…",
    sport: "Cartes de sport",
    sportHint: "Baseball, basket, football américain, hockey, football…",
  },
  games: {
    title: "Quels jeux ?",
    text: "Choisissez-en un et chaque nouvelle session y sera réglée, pour que Ximilar ne cherche que dans ce jeu. Choisissez-en plusieurs, ou aucun, pour choisir à chaque session.",
  },
  goals: {
    title: "Que voulez-vous savoir ?",
    text: "Cochez tout ce dont vous avez besoin.",
    identification: "Identification",
    identificationHint: "Nom, set et numéro de chaque carte",
    grading: "Gradation",
    gradingHint: "Une note complète : coins, bords, surface et centrage",
    condition: "État",
    conditionHint: "Un état rapide, p. ex. Near Mint, pour les annonces",
    prices: "Prix",
    pricesHint: "Prix du marché de chaque carte",
  },
  sources: {
    title: "Comment allez-vous importer les cartes ?",
    text: "Cochez tout ce que vous utilisez. Le premier s’ouvre avec Nouvelle session.",
    scanner: "Scanner",
    scannerHint: "Un scanner de documents avec chargeur, p. ex. RICOH fi-8170",
    folder: "Dossier d’images",
    folderHint: "Photos ou numérisations déjà sur cet ordinateur",
    watchFolder: "Dossier surveillé",
    watchFolderHint: "Les cartes sont analysées dès que de nouvelles images arrivent dans un dossier",
    csv: "Fichier CSV",
    csvHint: "Une liste de cartes ou de chemins d’images",
  },
  engine: {
    title: "Qu’est-ce qui doit identifier les cartes ?",
    ximilar: "API Ximilar",
    ximilarHint: "Identification précise, prix et gradation. Nécessite un jeton API et Internet.",
    local: "Modèle IA local",
    localHint:
      "Fonctionne sur cet ordinateur, hors ligne et gratuitement. Lit le nom, le numéro et le set et estime l’état ; moins précis.",
    token: "Jeton API",
    tokenSaved: "Déjà enregistré, laissez vide pour le conserver",
    tokenPaste: "Collez votre jeton",
    tokenHelp:
      "Vous le trouverez dans l’<link>Ximilar App</link>, sous votre profil. Vous pourrez l’ajouter plus tard dans Réglages.",
    pricesNeedApi: "Les prix nécessitent l’API Ximilar.",
    roughGrades:
      "Les notes et états du modèle local sont des estimations grossières d’un petit modèle, pas celles d’un gradeur.",
  },
};
export default messages;
