import type { Messages } from "..";
import type en from "../en/ebay";

const messages: Messages<typeof en> = {
  status: {
    draft: "Brouillon",
    listed: "En ligne",
    sold: "Vendue",
    ended: "Terminée",
  },
  badge: {
    draft: "eBay : brouillon",
    listed: "eBay : en ligne",
    sold: "eBay : vendue",
    ended: "eBay : terminée",
  },
  section: {
    notOnEbay: "Pas sur eBay",
    listing: "Annonce {id}",
    draftNote: "Une annonce non publiée : publiez-la, ou modifiez-la sur eBay.",
    publish: "Publier",
    endListing: "Terminer l’annonce",
    remove: "Retirer d’eBay",
  },
  list: {
    title: "Mettre en vente sur eBay",
    connectFirst: "Connectez d’abord votre compte eBay.",
    policiesFirst:
      "Choisissez d’abord vos politiques d’expédition, de paiement et de retour, ainsi que le lieu d’expédition des cartes.",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} sans prix", other: "{count} sans prix" },
    starting: "Démarrage…",
    createDrafts: { one: "Créer {count} brouillon", other: "Créer {count} brouillons" },
    intro:
      "Chaque carte devient une annonce non publiée (un brouillon) sur eBay, avec ses images, à prix fixe. Vérifiez les brouillons, puis publiez-les depuis la session — ou sur eBay. Les prix viennent de Ximilar quand il en a un.",
    introSandbox:
      "Chaque carte devient une annonce non publiée (un brouillon) dans la sandbox d’eBay, avec ses images, à prix fixe. Vérifiez les brouillons, puis publiez-les depuis la session — ou sur eBay. Les prix viennent de Ximilar quand il en a un.",
    condition: "État des cartes non gradées",
    gradedNote: "Les cartes dans un slab gradé sont mises en vente avec leur gradeur et leur note.",
    loadingCards: "Chargement des cartes…",
    noCards: "Aucune carte identifiée dans cette session pour l’instant.",
  },
  note: {
    sold: "Vendue sur eBay",
    live: "En ligne sur eBay — un nouveau prix s’applique à l’annonce · {note}",
    draft: "Déjà en brouillon sur eBay — la remettre en vente le met à jour · {note}",
  },
};
export default messages;
