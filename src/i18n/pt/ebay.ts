import type { Messages } from "..";
import type en from "../en/ebay";

const messages: Messages<typeof en> = {
  status: {
    draft: "Rascunho",
    listed: "Anunciada",
    sold: "Vendida",
    ended: "Encerrada",
  },
  badge: {
    draft: "eBay: rascunho",
    listed: "eBay: anunciada",
    sold: "eBay: vendida",
    ended: "eBay: encerrada",
  },
  section: {
    notOnEbay: "Não está no eBay",
    listing: "Anúncio {id}",
    draftNote: "Um anúncio não publicado: publique-o ou edite-o no eBay.",
    publish: "Publicar",
    endListing: "Encerrar anúncio",
    remove: "Remover do eBay",
  },
  list: {
    title: "Anunciar no eBay",
    connectFirst: "Conecte sua conta do eBay primeiro.",
    policiesFirst: "Primeiro escolha suas políticas de envio, pagamento e devolução e de onde as cartas são enviadas.",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} sem preço", other: "{count} sem preço" },
    starting: "Iniciando…",
    createDrafts: { one: "Criar {count} rascunho", other: "Criar {count} rascunhos" },
    intro:
      "Cada carta vira um anúncio não publicado (um rascunho) no eBay, com suas fotos, a preço fixo. Confira os rascunhos e publique-os pela sessão — ou no eBay. Os preços vêm do Ximilar quando ele tem um.",
    introSandbox:
      "Cada carta vira um anúncio não publicado (um rascunho) no sandbox do eBay, com suas fotos, a preço fixo. Confira os rascunhos e publique-os pela sessão — ou no eBay. Os preços vêm do Ximilar quando ele tem um.",
    condition: "Condição das cartas não graduadas",
    gradedNote: "Cartas em um slab graduado são anunciadas com a empresa avaliadora e a nota.",
    loadingCards: "Carregando cartas…",
    noCards: "Ainda não há cartas identificadas nesta sessão.",
  },
  note: {
    sold: "Vendida no eBay",
    live: "Ativa no eBay — um novo preço vale para o anúncio · {note}",
    draft: "Já é um rascunho no eBay — anunciá-la de novo o atualiza · {note}",
  },
};
export default messages;
