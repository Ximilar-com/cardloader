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
    comics: "Quadrinhos",
    comicsHint: "Gibis, revistas e mangás",
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
    crops: "Cartas recortadas",
    cropsHint: "Cada carta recortada e endireitada, salva em uma pasta",
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
    cropsNeedApi: "As cartas recortadas precisam da API da Ximilar: adicione seu token nas Configurações.",
    gradeTitle: "O que deve graduar as cartas?",
  },
  task: {
    title: "O que o Cardloader deve fazer com suas cartas?",
    text: "As novas sessões começam assim; você pode escolher de novo em cada sessão.",
    identify: "Identificá-las",
    identifyHint: "Nome, coleção, número e preço de cada carta, e uma nota se quiser",
    grade: "Só graduá-las",
    gradeHint: "Uma nota completa de cada carta (cantos, bordas, superfície, centralização), sem identificá-la",
    condition: "Só saber a condição",
    conditionHint: "A condição de cada carta, ex. Near Mint, para anúncios, sem identificá-la",
    crop: "Só recortá-las",
    cropHint: "Cada carta recortada da imagem e endireitada, salva como imagem em uma pasta",
  },
  token: {
    title: "Conecte a API da Ximilar",
    text: "O recorte é feito pela API da Ximilar: precisa de um token de API e internet.",
  },
};
export default messages;
