import type { Messages } from "..";
import type en from "../en/market";

const messages: Messages<typeof en> = {
  conditionHint: {
    excellent: "légèrement jouée",
    veryGood: "modérément jouée",
    poor: "très jouée",
  },
  priced: {
    allCards: "Toutes les cartes",
    canBeListed: {
      one: "{count} carte peut être mise en vente",
      other: "{count} cartes peuvent être mises en vente",
    },
    canBeExported: { one: "{count} carte peut être exportée", other: "{count} cartes peuvent être exportées" },
    adjustPrices: "Ajuster les prix",
    priceUsd: "Prix (USD)",
    percentOfMarket: "<field></field> % du prix du marché",
    percentLabel: "Pourcentage du prix du marché",
    endIn99: "finir en ,99",
    atLeast: "au moins <field></field> $",
    lowestPrice: "Prix minimum",
    apply: "Appliquer aux cartes choisies",
    choose: "Choisir n° {seq}",
    price: "Prix",
    priceOf: "Prix du n° {seq}",
  },
  listingType: {
    auction: "Enchère",
    auctionHint: "le prix est l’enchère de départ",
    buyItNow: "Achat immédiat",
    buyItNowHint: "un prix fixe",
  },
  shopifyFile: {
    products: "Produits",
    productsHint: "nouveaux produits avec variantes",
    inventory: "Mise à jour du stock",
    inventoryHint: "produits existants",
  },
  shopifyGrouping: {
    byCard: "Un produit par carte",
    byCardHint: "les états sont des variantes",
    perCard: "Un produit par carte numérisée",
  },
  export: {
    title: "Exporter pour {name}",
    uploadingProgress: "Envoi des images… {done}/{total}",
    preparingProgress: "Préparation… {done}/{total}",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} sans prix", other: "{count} sans prix" },
    stop: "Arrêter",
    exporting: "Exportation…",
    exportCsv: "Exporter en CSV…",
    tcgplayerIntro:
      "Une liste des cartes à collectionner non gradées dans les termes de TCGplayer — gamme de produits, set, nom, numéro, état et votre prix — avec le lien produit TCGplayer de chaque carte. TCGplayer n’importe que les fichiers de son propre export Seller Portal : ajoutez donc les cartes dans Seller Portal à partir de cette liste. Les cartes gradées s’y vendent en Listings with Photos, une par une, et TCGplayer n’a pas de cartes de sport.",
    shopifyInventoryIntro:
      "Le fichier de stock de Shopify (Products → Inventory → Import) : fixe le stock de chaque produit à l’emplacement au nombre d’exemplaires présents ici. Il retrouve les produits créés par l’export de produits de cette session — même regroupement — par leurs handles et leurs états.",
    shopifyProductsIntro:
      "Le fichier de produits de Shopify (Products → Import). Les cartes sont retrouvées par leurs handles : cochez « Overwrite products with matching handles » pour mettre à jour les produits importés auparavant. Les slabs gradés sont des produits à part. Dans les boutiques avec plusieurs emplacements, réglez ensuite le stock avec la mise à jour du stock.",
    whatnotIntro:
      "Le fichier d’annonces en masse de Whatnot : importez-le dans Seller Hub (Inventory → Import from CSV) pour des brouillons d’annonces, ou dans un show (Add → Create Temporary Listing → Upload CSV). Les slabs gradés sont mis en vente comme Graded avec le gradeur et la note dans le titre et la description. Les prix sont en dollars entiers, arrondis au supérieur.",
    type: "Type",
    listingType: "Type d’annonce",
    shippingProfile: "Profil d’expédition",
    file: "Fichier",
    shopifyFile: "Fichier Shopify",
    products: "Produits",
    condition: "État des cartes non gradées",
    conditionNote: "Les cartes dont Ximilar a évalué l’état gardent le leur.",
    location: "Emplacement",
    locationPlaceholder: "p. ex. Emplacement de la boutique",
    locationNote: "Exactement comme nommé dans Shopify (Settings → Locations).",
    uploadPictures: "Envoyer les images (ImgBB)",
    publishActive: "Publier (actives) plutôt qu’en brouillons",
    offerable: "Les acheteurs peuvent faire des offres",
    noImgbbKey:
      "{name} télécharge les images depuis des liens. Ajoutez une clé API ImgBB dans Réglages pour les envoyer ; sans clé, ajoutez les photos dans {name}.",
    loadingCards: "Chargement des cartes…",
    noCards: "Aucune carte identifiée dans cette session pour l’instant.",
    variantNote:
      "Les exemplaires d’une carte dans le même état deviennent une seule variante, au plus élevé de leurs prix.",
  },
  result: {
    title: "Exporté pour {name}",
    showFile: "Afficher le fichier",
    done: "Terminé",
    products: { one: "{count} produit", other: "{count} produits" },
    variants: { one: "{count} variante", other: "{count} variantes" },
    productsVariants: "{products} ({variants})",
    saved: "Fichier enregistré dans <path></path> : {what}.",
    withoutPictures: {
      one: "{count} sans images : ajoutez-les dans {name} avant de publier.",
      other: "{count} sans images : ajoutez-les dans {name} avant de publier.",
    },
    leftOut: "Exclues :",
  },
};
export default messages;
