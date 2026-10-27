import type { Messages } from "..";
import type en from "../en/market";

const messages: Messages<typeof en> = {
  conditionHint: {
    excellent: "ligeramente jugada",
    veryGood: "moderadamente jugada",
    poor: "muy jugada",
  },
  priced: {
    allCards: "Todas las cartas",
    canBeListed: { one: "Se puede anunciar {count} carta", other: "Se pueden anunciar {count} cartas" },
    canBeExported: { one: "Se puede exportar {count} carta", other: "Se pueden exportar {count} cartas" },
    adjustPrices: "Ajustar precios",
    priceUsd: "Precio (USD)",
    percentOfMarket: "<field></field> % del precio de mercado",
    percentLabel: "Porcentaje del precio de mercado",
    endIn99: "terminar en .99",
    atLeast: "como mínimo $<field></field>",
    lowestPrice: "Precio mínimo",
    apply: "Aplicar a las cartas elegidas",
    choose: "Elegir #{seq}",
    price: "Precio",
    priceOf: "Precio de #{seq}",
  },
  listingType: {
    auction: "Subasta",
    auctionHint: "el precio es la puja inicial",
    buyItNow: "Buy it Now",
    buyItNowHint: "un precio fijo",
  },
  shopifyFile: {
    products: "Productos",
    productsHint: "productos nuevos con variantes",
    inventory: "Actualizar inventario",
    inventoryHint: "productos existentes",
  },
  shopifyGrouping: {
    byCard: "Un producto por carta",
    byCardHint: "los estados son variantes",
    perCard: "Un producto por carta escaneada",
  },
  export: {
    title: "Exportar para {name}",
    uploadingProgress: "Subiendo fotos… {done}/{total}",
    preparingProgress: "Preparando… {done}/{total}",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} sin precio", other: "{count} sin precio" },
    stop: "Detener",
    exporting: "Exportando…",
    exportCsv: "Exportar CSV…",
    tcgplayerIntro:
      "Una lista de las cartas coleccionables sin gradar en los términos de TCGplayer (línea de producto, set, nombre, número, estado y tu precio) con el enlace al producto de TCGplayer de cada carta. TCGplayer solo importa archivos exportados desde su propio Seller Portal, así que añade las cartas en Seller Portal a partir de esta lista. Allí las cartas gradadas se venden como Listings with Photos, una a una, y TCGplayer no tiene cartas deportivas.",
    shopifyInventoryIntro:
      "El archivo de inventario de Shopify (Productos → Inventario → Importar): fija el stock de cada producto en la ubicación al número de copias que hay aquí. Encuentra los productos que creó la exportación de productos de esta sesión (con la misma agrupación) por sus identificadores (handles) y estados.",
    shopifyProductsIntro:
      "El archivo de productos de Shopify (Productos → Importar). Las cartas se vuelven a encontrar por sus identificadores (handles), así que marca «Sobrescribir productos con identificadores coincidentes» para actualizar productos importados antes. Los slabs gradados son productos propios. En tiendas con varias ubicaciones, fija después el stock con la actualización de inventario.",
    whatnotIntro:
      "El archivo de anuncios masivos de Whatnot: impórtalo en Seller Hub (Inventory → Import from CSV) para crear borradores, o en un show (Add → Create Temporary Listing → Upload CSV). Los slabs gradados se anuncian como Graded con la empresa de gradación y la nota en el título y la descripción. Los precios son dólares enteros, redondeados hacia arriba.",
    type: "Tipo",
    listingType: "Tipo de anuncio",
    shippingProfile: "Perfil de envío",
    file: "Archivo",
    shopifyFile: "Archivo de Shopify",
    products: "Productos",
    condition: "Estado de las cartas sin gradar",
    conditionNote: "Las cartas cuyo estado gradó Ximilar conservan el suyo.",
    location: "Ubicación",
    locationPlaceholder: "p. ej. Ubicación de la tienda",
    locationNote: "Exactamente como se llama en Shopify (Configuración → Ubicaciones).",
    uploadPictures: "Subir las fotos (ImgBB)",
    publishActive: "Publicar (activos) en vez de borradores",
    offerable: "Los compradores pueden hacer ofertas",
    noImgbbKey:
      "{name} descarga las fotos desde enlaces. Añade una clave de API de ImgBB en Ajustes para subirlas; sin ella, añade las fotos en {name}.",
    loadingCards: "Cargando cartas…",
    noCards: "Aún no hay cartas identificadas en esta sesión.",
    variantNote: "Las copias de una carta en el mismo estado forman una variante, al mayor de sus precios.",
  },
  result: {
    title: "Exportado para {name}",
    showFile: "Mostrar archivo",
    done: "Listo",
    products: { one: "{count} producto", other: "{count} productos" },
    variants: { one: "{count} variante", other: "{count} variantes" },
    productsVariants: "{products} ({variants})",
    saved: "{what} guardado en <path></path>.",
    withoutPictures: {
      one: "{count} sin fotos: añádelas en {name} antes de publicar.",
      other: "{count} sin fotos: añádelas en {name} antes de publicar.",
    },
    leftOut: "Excluidas:",
  },
};
export default messages;
