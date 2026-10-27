import type { Messages } from "..";
import type en from "../en/cards";

const messages: Messages<typeof en> = {
  title: "Cartes",
  total: { one: "<b>{count}</b> carte", other: "<b>{count}</b> cartes" },
  intro:
    "Toutes les cartes identifiées de vos sessions sur cet ordinateur. Recherchez par nom, set, numéro, emplacement ou notes.",
  filter: {
    all: "Toutes",
    review: "À vérifier",
    graded: "Gradées",
    duplicates: "Doublons",
    agent: "Par l’IA",
  },
  selected: "Sélection : {count}",
  accept: "Accepter",
  accepted: { one: "{count} correspondance acceptée", other: "{count} correspondances acceptées" },
  unaccept: "Annuler l’acceptation",
  unacceptTitle: "Retirer les acceptations",
  unaccepted: { one: "{count} acceptation retirée", other: "{count} acceptations retirées" },
  setLocation: "Définir l’emplacement…",
  turnUpright: "Remettre à l’endroit",
  turnUprightTitle: "Remettre à l’endroit les cartes que Ximilar a trouvées à l’envers, recto et verso",
  clearSelection: "Effacer la sélection",
  searchPlaceholder: "Nom, set, numéro, emplacement…",
  search: "Rechercher des cartes",
  showingNewest: "Affichage des {count} plus récentes — recherchez ou filtrez pour trouver les autres.",
  noMatch: "Aucune carte ne correspond.",
  empty: "Aucune carte identifiée pour l’instant.",
  goToSessions: "Aller aux sessions",
  position: "Carte {at} sur {count}",
  turned: "Remise à l’endroit",
  inSession: "Dans la session « {name} »",
  location: {
    title: { one: "Emplacement de {count} carte", other: "Emplacement de {count} cartes" },
    hint: "Où elles sont rangées, p. ex. « Boîte 4 » ou « Classeur 2, page 7 ». C’est consultable par la recherche et inclus dans l’export CSV.",
    confirm: "Définir l’emplacement",
    done: { one: "{count} carte désormais à {location}", other: "{count} cartes désormais à {location}" },
  },
};
export default messages;
