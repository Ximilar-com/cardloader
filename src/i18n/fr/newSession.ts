import type { Messages } from "..";
import type en from "../en/newSession";

const messages: Messages<typeof en> = {
  title: {
    new: "Nouvelle session",
    edit: "Réglages de la session",
    duplicate: "Dupliquer la session",
  },
  copyName: "{name} (copie)",
  importing: "Importation…",
  import: "Importer",
  create: "Créer",
  importAndRun: "Importer et lancer",
  importAndRunTitle: "Importer, puis analyser les images et envoyer selon les réglages ci-dessous",
  createAndRun: "Créer et lancer",
  needsToken: "Définissez votre jeton API Ximilar dans Réglages avant de lancer une session.",
  needsModel: "Téléchargez un modèle local dans Réglages avant de lancer cette session.",
  editNote:
    "Les modifications s’appliquent aux cartes analysées à partir de maintenant. Les cartes déjà analysées gardent leurs résultats ; utilisez <b>Dupliquer</b> pour analyser à nouveau la même source avec d’autres réglages.",
  name: "Nom",
  source: "Source",
  sources: {
    folder: "Analyser les images d’un dossier",
    watch_folder: "Analyser les images à mesure qu’une autre app les enregistre",
    scanner: "Numériser une pile de cartes (p. ex. fi-8170)",
    csv: "Importer des cartes ou des chemins d’images depuis un tableur",
  },
  browse: "Parcourir",
  csv: {
    label: "Fichier CSV",
    help: "Une carte par ligne. Les lignes avec un nom de carte (et set, numéro, prix, note…) sont importées comme cartes identifiées ; les lignes avec seulement un chemin d’image sont analysées. Un export CSV de cardloader se réimporte tel quel.",
    reanalyze: "Analyser à nouveau les images des {count} lignes identifiées",
    summary: "<b>{rows}</b> : {cards} à importer, {images} à analyser.",
    summarySkipped: "<b>{rows}</b> : {cards} à importer, {images} à analyser, {skipped} vides ignorées.",
    rows: { one: "{count} ligne", other: "{count} lignes" },
    identifiedCards: { one: "{count} carte identifiée", other: "{count} cartes identifiées" },
    images: { one: "{count} image", other: "{count} images" },
    missingImages: {
      one: "{count} ligne pointe vers des images manquantes.",
      other: "{count} lignes pointent vers des images manquantes.",
    },
    columns: "Colonnes : {columns}",
  },
  folder: {
    label: "Dossier",
    chooseWatch: "Choisissez le dossier où votre app de numérisation enregistre les images.",
    choose: "Choisissez le dossier contenant les images de vos cartes.",
    noImages: "Aucune image dans ce dossier (JPEG, PNG, WebP, TIFF ou BMP).",
    found: { one: "{count} image trouvée", other: "{count} images trouvées" },
    foundWatching: {
      one: "{count} image trouvée — les nouvelles images seront prises en compte pendant la session",
      other: "{count} images trouvées — les nouvelles images seront prises en compte pendant la session",
    },
  },
  cost: {
    local: "identifiées sur cet ordinateur, sans appel API",
    calls: { one: "environ {count} appel API", other: "environ {count} appels API" },
    callsCloud: {
      one: "environ {count} appel API pour Cards Cloud",
      other: "environ {count} appels API pour Cards Cloud",
    },
    callsMagicAi: {
      one: "environ {count} appel API (Magic AI peut en ajouter)",
      other: "environ {count} appels API (Magic AI peut en ajouter)",
    },
  },
  scanner: {
    label: "Scanner",
    searching: "Recherche…",
    noneFound: "Aucun scanner trouvé",
    resolution: "Résolution",
    dpiWhenNotGrading: "300 dpi sans gradation.",
    sides: "Faces",
    duplex: "Recto + verso",
    frontOnly: "Recto seul",
    multifeed: "Détection de double alimentation",
    shared: "Ce sont vos réglages du scanner : les modifier ici les modifie pour toutes les sessions de numérisation.",
  },
  identifyWith: "Identifier avec",
  model: "Modèle",
  noModel: "Aucun modèle téléchargé",
  notDownloaded: "non téléchargé",
  localNote:
    "Fonctionne sur cet ordinateur : lit le nom, le numéro de collection et l’année imprimés sur chaque carte, et estime les notes et l’état. Pas de prix ni d’autres candidats.",
  identifyAs: "Identifier comme",
  grading: "Gradation",
  imagesPerCard: "Images par carte",
  conditionScale: "Échelle d’état",
  priceStats: "Statistiques de prix",
  readSlabs: "Lire les étiquettes des slabs gradés",
  syncRemote: "Synchroniser avec Ximilar Cards Cloud",
  known: {
    title: "Plus rapide et plus précis",
    intro:
      "Indiquez à Ximilar ce que vous savez déjà des cartes, pour qu’il n’ait pas à le déduire. Ne cochez que ce qui est vrai pour chaque image : une valeur fausse dégrade les correspondances.",
    skipDetection: "Ignorer la détection",
    skipDetectionHint:
      "Pour les images déjà recadrées sur la carte : chaque image est envoyée comme la carte elle-même, Ximilar ne la cherche donc pas dans l’image.",
    skipDetectionSlab: "Les étiquettes des slabs sont trouvées par la détection : elles risquent de ne pas être lues.",
    everyCard: {
      tcg: "Chaque carte est une carte à collectionner",
      sport: "Chaque carte est une carte de sport",
      comics: "Chaque carte est un comic",
      slab: "Chaque carte est un slab gradé",
    },
    cardType: "Type de carte",
    sendsCategory: "Envoie la catégorie « {category} ».",
    chooseTypeFirst: "Choisissez d’abord un type dans « Identifier comme » ; Détection auto laisse Ximilar décider.",
    frontSide: "Recto",
    frontSideHint:
      "Les images montrent le recto (les versos envoyés avec « le recto et le verso » sont marqués comme verso).",
    upright: "À l’endroit",
    uprightHint:
      "Aucune carte n’est tournée ni à l’envers. Sinon, Ximilar remet chaque carte à l’endroit avant de l’identifier.",
    game: "Jeu",
    alphabet: "Alphabet",
    gameNote:
      "Le jeu limite la recherche à ses cartes ; l’alphabet (p. ex. japonais pour les impressions japonaises) évite qu’une carte corresponde à sa version dans une autre langue.",
    gameNoAlphabet:
      "Quand un jeu est choisi, Ximilar ne lit plus l’alphabet et suppose le latin : choisissez-le pour les impressions dans d’autres écritures.",
    sport: "Sport",
    sportNote: "Limite la recherche aux cartes de ce sport.",
  },
  subcategory: {
    anyGame: "Tous les jeux",
    anySport: "Tous les sports",
    readFromCard: "Ximilar le lit sur la carte",
  },
  alphabets: {
    detect: "Détecter",
    latin: "Latin",
    latinHint: "anglais, langues européennes",
    japanese: "Japonais",
    chinese: "Chinois",
    korean: "Coréen",
    russian: "Russe",
    thai: "Thaï",
  },
  more: {
    label: "Plus d’options",
    summary: "— ce que vous savez des cartes, recto et verso, Magic AI, endpoint personnalisé",
  },
  bothSides: {
    label: "Identifier à partir du recto et du verso",
    csv: "Utilise la colonne d’image du verso du CSV.",
    paired:
      "Les images nommées <code>card_F.jpg</code> et <code>card_B.jpg</code> (ou front/back) sont associées par nom, sinon les images consécutives.",
    why: "Le verso est envoyé à la reconnaissance avec son recto — cela aide pour les cartes de sport, dont le verso indique le joueur, l’année et le numéro.",
    frontOnly: "L’identification « {type} » ne lit que le recto : leurs versos se ressemblent tous.",
    magicAi: "Magic AI pour les cartes de sport",
    magicAiHint:
      "Quand une carte de sport n’est pas dans la base de Ximilar, un modèle d’IA l’identifie à partir de ses images. Des crédits API supplémentaires ne sont utilisés que lorsqu’il intervient.",
    magicAiSportOnly: "Ne s’applique qu’aux cartes de sport.",
    autoNote:
      "Détection auto identifie une carte en un appel ; une carte de sport est ensuite aussi envoyée à Sports ID pour ces options (un appel de plus).",
    customEndpoint: "Endpoint personnalisé (facultatif)",
    customEndpointHint:
      "Un endpoint de reconnaissance que Ximilar a configuré pour vous. Il reçoit la même requête que l’endpoint standard (recto et verso si choisi ci-dessus), et ses réponses sont lues comme « {type} ».",
    customEndpointHintAuto:
      "Un endpoint de reconnaissance que Ximilar a configuré pour vous. Il reçoit la même requête que l’endpoint standard (recto et verso si choisi ci-dessus), et ses réponses sont lues comme le type de carte détecté.",
  },
  options: {
    dpiIdentification: "identification",
    dpiGrading: "gradation",
    tradingCards: "cartes à collectionner",
    sportsCards: "cartes de sport",
    apiOnly: "API uniquement",
    multifeedOffHint: "recommandé pour les cartes",
    multifeedDefault: "Par défaut du pilote",
    single: "Une image par carte",
    frontBack: "Paires recto + verso",
  },
  openSettings: "Ouvrir les réglages",
  remote: {
    label: "Poursuivre une session Cards Cloud (facultatif)",
    placeholder: "ID ou lien de la session",
    untitled: "Sans titre",
    found: { one: "« {name} » · {count} carte", other: "« {name} » · {count} cartes" },
    paste: "Collez l’ID de la session ou son lien depuis la Ximilar App.",
    added: "{found}. Les nouvelles cartes y sont ajoutées.",
    addedSkips:
      "{found}. Les nouvelles cartes y sont ajoutées ; les images qu’elle contient déjà (reconnues par nom de fichier) sont ignorées.",
    lookingUp: "Recherche…",
    hint: "Ajoute les cartes à une session commencée plus tôt, p. ex. sur un autre ordinateur.",
    hintSkips:
      "Ajoute les cartes à une session commencée plus tôt, p. ex. sur un autre ordinateur ; les images qu’elle contient déjà (reconnues par nom de fichier) sont ignorées.",
  },
};
export default messages;
