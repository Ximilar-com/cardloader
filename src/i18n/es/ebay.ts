import type { Messages } from "..";
import type en from "../en/ebay";

const messages: Messages<typeof en> = {
  status: {
    draft: "Borrador",
    listed: "Publicado",
    sold: "Vendido",
    ended: "Finalizado",
  },
  badge: {
    draft: "eBay: borrador",
    listed: "eBay: publicado",
    sold: "eBay: vendido",
    ended: "eBay: finalizado",
  },
  section: {
    notOnEbay: "No está en eBay",
    listing: "Anuncio {id}",
    draftNote: "Un anuncio sin publicar: publícalo o edítalo en eBay.",
    publish: "Publicar",
    endListing: "Finalizar anuncio",
    remove: "Quitar de eBay",
  },
  list: {
    title: "Anunciar en eBay",
    connectFirst: "Primero conecta tu cuenta de eBay.",
    policiesFirst: "Primero elige tus políticas de envío, pago y devolución, y desde dónde se envían las cartas.",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} sin precio", other: "{count} sin precio" },
    starting: "Iniciando…",
    createDrafts: { one: "Crear {count} borrador", other: "Crear {count} borradores" },
    intro:
      "Cada carta se convierte en un anuncio sin publicar (un borrador) en eBay, con sus fotos y a precio fijo. Revisa los borradores y luego publícalos desde la sesión o en eBay. Los precios vienen de Ximilar cuando los tiene.",
    introSandbox:
      "Cada carta se convierte en un anuncio sin publicar (un borrador) en el sandbox de eBay, con sus fotos y a precio fijo. Revisa los borradores y luego publícalos desde la sesión o en eBay. Los precios vienen de Ximilar cuando los tiene.",
    condition: "Estado de las cartas sin gradar",
    gradedNote: "Las cartas en un slab gradado se anuncian con su empresa de gradación y su nota.",
    loadingCards: "Cargando cartas…",
    noCards: "Aún no hay cartas identificadas en esta sesión.",
  },
  note: {
    sold: "Vendida en eBay",
    live: "Publicada en eBay: un precio nuevo se aplica al anuncio · {note}",
    draft: "Ya es un borrador en eBay: anunciarla de nuevo lo actualiza · {note}",
  },
};
export default messages;
