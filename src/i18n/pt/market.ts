import type { Messages } from "..";
import type en from "../en/market";

const messages: Messages<typeof en> = {
  conditionHint: {
    excellent: "levemente jogada",
    veryGood: "moderadamente jogada",
    poor: "muito jogada",
  },
  priced: {
    allCards: "Todas as cartas",
    canBeListed: { one: "{count} carta pode ser anunciada", other: "{count} cartas podem ser anunciadas" },
    canBeExported: { one: "{count} carta pode ser exportada", other: "{count} cartas podem ser exportadas" },
    adjustPrices: "Ajustar preços",
    priceUsd: "Preço (USD)",
    percentOfMarket: "<field></field> % do preço de mercado",
    percentLabel: "Porcentagem do preço de mercado",
    endIn99: "terminar em .99",
    atLeast: "no mínimo $<field></field>",
    lowestPrice: "Preço mínimo",
    apply: "Aplicar às cartas escolhidas",
    choose: "Escolher #{seq}",
    price: "Preço",
    priceOf: "Preço de #{seq}",
  },
  listingType: {
    auction: "Leilão",
    auctionHint: "o preço é o lance inicial",
    buyItNow: "Compra imediata",
    buyItNowHint: "um preço fixo",
  },
  shopifyFile: {
    products: "Produtos",
    productsHint: "novos produtos com variantes",
    inventory: "Atualização de estoque",
    inventoryHint: "produtos existentes",
  },
  shopifyGrouping: {
    byCard: "Um produto por carta",
    byCardHint: "as condições são variantes",
    perCard: "Um produto por carta escaneada",
  },
  export: {
    title: "Exportar para {name}",
    uploadingProgress: "Enviando fotos… {done}/{total}",
    preparingProgress: "Preparando… {done}/{total}",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} sem preço", other: "{count} sem preço" },
    stop: "Parar",
    exporting: "Exportando…",
    exportCsv: "Exportar CSV…",
    tcgplayerIntro:
      "Uma lista das cartas colecionáveis não graduadas nos termos do TCGplayer — linha de produto, coleção, nome, número, condição e seu preço — com o link do produto no TCGplayer de cada carta. O TCGplayer só importa arquivos exportados do seu próprio Seller Portal, então adicione as cartas no Seller Portal a partir desta lista. Cartas graduadas são vendidas lá como Listings with Photos, uma a uma, e o TCGplayer não tem cartas esportivas.",
    shopifyInventoryIntro:
      "O arquivo de estoque do Shopify (Produtos → Estoque → Importar): define o estoque de cada produto no local como o número de cópias aqui. Ele encontra os produtos criados pela exportação de produtos desta sessão — com o mesmo agrupamento — pelos handles e condições.",
    shopifyProductsIntro:
      "O arquivo de produtos do Shopify (Produtos → Importar). As cartas são reencontradas pelos handles, então marque “Substituir produtos com handles correspondentes” para atualizar produtos importados antes. Slabs graduados são produtos próprios. Em lojas com vários locais, defina o estoque depois com a atualização de estoque.",
    whatnotIntro:
      "O arquivo de anúncios em massa do Whatnot: importe-o no Seller Hub (Inventory → Import from CSV) para criar rascunhos de anúncio, ou em um show (Add → Create Temporary Listing → Upload CSV). Slabs graduados são anunciados como Graded, com a empresa avaliadora e a nota no título e na descrição. Os preços são em dólares inteiros, arredondados para cima.",
    type: "Tipo",
    listingType: "Tipo de anúncio",
    shippingProfile: "Perfil de envio",
    file: "Arquivo",
    shopifyFile: "Arquivo do Shopify",
    products: "Produtos",
    condition: "Condição das cartas não graduadas",
    conditionNote: "Cartas cuja condição o Ximilar avaliou mantêm a delas.",
    location: "Local",
    locationPlaceholder: "ex. Local da loja",
    locationNote: "Exatamente como está no Shopify (Configurações → Locais).",
    uploadPictures: "Enviar as fotos (ImgBB)",
    publishActive: "Publicar (ativos) em vez de rascunhos",
    offerable: "Compradores podem fazer ofertas",
    noImgbbKey:
      "O {name} baixa as fotos a partir de links. Adicione uma chave de API do ImgBB em Configurações para enviá-las; sem ela, adicione as fotos no {name}.",
    loadingCards: "Carregando cartas…",
    noCards: "Ainda não há cartas identificadas nesta sessão.",
    variantNote: "Cópias de uma carta na mesma condição viram uma única variante, pelo maior dos preços.",
  },
  result: {
    title: "Exportado para {name}",
    showFile: "Mostrar arquivo",
    done: "Concluir",
    products: { one: "{count} produto", other: "{count} produtos" },
    variants: { one: "{count} variante", other: "{count} variantes" },
    productsVariants: "{products} ({variants})",
    saved: "{what} salvo em <path></path>.",
    withoutPictures: {
      one: "{count} sem fotos: adicione-as no {name} antes de publicar.",
      other: "{count} sem fotos: adicione-as no {name} antes de publicar.",
    },
    leftOut: "Deixadas de fora:",
  },
};
export default messages;
