import type { Messages } from "..";
import type en from "../en/wizard";

const messages: Messages<typeof en> = {
  title: "Configurar o Cardloader",
  progress: "Etapa {step} de {steps} · você pode mudar tudo depois em Configurações",
  alwaysIncluded: "Sempre incluído",
  language: {
    title: "Escolha seu idioma",
    text: "O Cardloader vai usá-lo a partir de agora.",
  },
  interests: {
    title: "O que você vai escanear?",
    text: "Escolha tudo o que você coleciona.",
    tcg: "Jogos de cartas colecionáveis",
    tcgHint: "Pokémon, Magic, Yu-Gi-Oh!, One Piece, Lorcana…",
    sport: "Cartas esportivas",
    sportHint: "Beisebol, basquete, futebol americano, hóquei, futebol…",
  },
  games: {
    title: "Quais jogos?",
    text: "Escolha um e toda nova sessão já vem com ele, para o Ximilar buscar só cartas desse jogo. Escolha vários, ou nenhum, para decidir em cada sessão.",
  },
  goals: {
    title: "O que você quer descobrir?",
    text: "Escolha tudo o que precisar.",
    identification: "Identificação",
    identificationHint: "Nome, coleção e número de cada carta",
    grading: "Graduação",
    gradingHint: "Uma nota completa: cantos, bordas, superfície e centralização",
    condition: "Condição",
    conditionHint: "Uma condição rápida, ex. Near Mint, para anúncios",
    prices: "Preços",
    pricesHint: "Preços de mercado de cada carta",
  },
  sources: {
    title: "Como você vai trazer as cartas?",
    text: "Escolha tudo o que usa. A primeira opção abre com Nova sessão.",
    scanner: "Scanner",
    scannerHint: "Um scanner de documentos com alimentador, ex. RICOH fi-8170",
    folder: "Pasta de imagens",
    folderHint: "Fotos ou digitalizações que já estão neste computador",
    watchFolder: "Pasta monitorada",
    watchFolderHint: "As cartas são analisadas conforme novas imagens chegam a uma pasta",
    csv: "Arquivo CSV",
    csvHint: "Uma lista de cartas ou caminhos de imagens",
  },
  engine: {
    title: "O que deve identificar as cartas?",
    ximilar: "API do Ximilar",
    ximilarHint: "Identificação precisa, preços e graduação. Precisa de um token de API e de internet.",
    local: "Modelo de IA local",
    localHint:
      "Roda neste computador, offline e de graça. Lê nome, número e coleção e estima a condição; é menos preciso.",
    token: "Token de API",
    tokenSaved: "Já salvo, deixe vazio para mantê-lo",
    tokenPaste: "Cole seu token",
    tokenHelp:
      "Você o encontra no <link>Ximilar App</link>, no seu perfil. Também pode adicioná-lo depois em Configurações.",
    pricesNeedApi: "Os preços precisam da API do Ximilar.",
    roughGrades:
      "As notas e condições do modelo local são estimativas aproximadas de um modelo pequeno, não de um avaliador.",
  },
};
export default messages;
