import type { Messages } from "..";
import type en from "../en/cards";

const messages: Messages<typeof en> = {
  title: "Cartas",
  total: { one: "<b>{count}</b> carta", other: "<b>{count}</b> cartas" },
  intro:
    "Todas as cartas identificadas das suas sessões neste computador. Pesquise por nome, coleção, número, local ou notas.",
  filter: {
    all: "Todas",
    review: "Para revisar",
    graded: "Graduadas",
    duplicates: "Duplicadas",
    agent: "Por IA",
  },
  selected: "{count} selecionadas",
  accept: "Aceitar",
  accepted: { one: "{count} correspondência aceita", other: "{count} correspondências aceitas" },
  unaccept: "Desfazer aceite",
  unacceptTitle: "Desfazer os aceites",
  unaccepted: {
    one: "Aceite desfeito em {count} correspondência",
    other: "Aceite desfeito em {count} correspondências",
  },
  setLocation: "Definir local…",
  turnUpright: "Endireitar",
  turnUprightTitle: "Endireitar as cartas que o Ximilar achou de cabeça para baixo, frente e verso",
  clearSelection: "Limpar a seleção",
  searchPlaceholder: "Nome, coleção, número, local…",
  search: "Pesquisar cartas",
  showingNewest: "Mostrando as {count} mais recentes — pesquise ou filtre para achar as outras.",
  noMatch: "Nenhuma carta encontrada.",
  empty: "Nenhuma carta identificada ainda.",
  goToSessions: "Ir para as sessões",
  position: "Carta {at} de {count}",
  turned: "Endireitada",
  inSession: "Na sessão “{name}”",
  location: {
    title: { one: "Local de {count} carta", other: "Local de {count} cartas" },
    hint: "Onde elas ficam guardadas, ex. “Caixa 4” ou “Fichário 2, página 7”. Dá para pesquisar e vai para a exportação CSV.",
    confirm: "Definir local",
    done: { one: "{count} carta agora em {location}", other: "{count} cartas agora em {location}" },
  },
};
export default messages;
