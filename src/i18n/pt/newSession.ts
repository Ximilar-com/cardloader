import type { Messages } from "..";
import type en from "../en/newSession";

const messages: Messages<typeof en> = {
  title: {
    new: "Nova sessão",
    edit: "Configurações da sessão",
    duplicate: "Duplicar sessão",
  },
  copyName: "{name} (cópia)",
  importing: "Importando…",
  import: "Importar",
  create: "Criar",
  importAndRun: "Importar e executar",
  importAndRunTitle: "Importar, depois analisar as imagens e enviar conforme abaixo",
  createAndRun: "Criar e executar",
  needsToken: "Defina seu token de API do Ximilar em Configurações antes de executar uma sessão.",
  needsModel: "Baixe um modelo local em Configurações antes de executar esta sessão.",
  editNote:
    "As mudanças valem para as cartas analisadas daqui em diante. As cartas já analisadas mantêm seus resultados; use <b>Duplicar</b> para analisar a mesma origem de novo com outras configurações.",
  name: "Nome",
  source: "Origem",
  sources: {
    folder: "Analisar as imagens de uma pasta",
    watch_folder: "Analisar imagens conforme outro app as salva",
    scanner: "Escanear uma pilha de cartas (ex. fi-8170)",
    csv: "Importar cartas ou caminhos de imagens de uma planilha",
  },
  browse: "Procurar",
  csv: {
    label: "Arquivo CSV",
    help: "Uma carta por linha. Linhas com nome da carta (e coleção, número, preço, nota…) são importadas como cartas identificadas; linhas só com um caminho de imagem são analisadas. Uma exportação CSV do cardloader volta exatamente como era.",
    reanalyze: "Analisar de novo as imagens das {count} linhas identificadas",
    summary: "<b>{rows}</b>: {cards} para importar, {images} para analisar.",
    summarySkipped: "<b>{rows}</b>: {cards} para importar, {images} para analisar, {skipped} vazias ignoradas.",
    rows: { one: "{count} linha", other: "{count} linhas" },
    identifiedCards: { one: "{count} carta identificada", other: "{count} cartas identificadas" },
    images: { one: "{count} imagem", other: "{count} imagens" },
    missingImages: {
      one: "{count} linha aponta para imagens que não existem.",
      other: "{count} linhas apontam para imagens que não existem.",
    },
    columns: "Colunas: {columns}",
  },
  folder: {
    label: "Pasta",
    chooseWatch: "Escolha a pasta onde seu app de digitalização salva as imagens.",
    choose: "Escolha a pasta com as imagens das suas cartas.",
    noImages: "Nenhuma imagem nesta pasta (JPEG, PNG, WebP, TIFF ou BMP).",
    found: { one: "{count} imagem encontrada", other: "{count} imagens encontradas" },
    foundWatching: {
      one: "{count} imagem encontrada — novas imagens serão incluídas enquanto a sessão estiver em execução",
      other: "{count} imagens encontradas — novas imagens serão incluídas enquanto a sessão estiver em execução",
    },
  },
  cost: {
    local: "identificadas neste computador, sem chamadas à API",
    calls: { one: "cerca de {count} chamada à API", other: "cerca de {count} chamadas à API" },
    callsCloud: {
      one: "cerca de {count} chamada à API para o Cards Cloud",
      other: "cerca de {count} chamadas à API para o Cards Cloud",
    },
    callsMagicAi: {
      one: "cerca de {count} chamada à API (o Magic AI pode somar mais)",
      other: "cerca de {count} chamadas à API (o Magic AI pode somar mais)",
    },
  },
  scanner: {
    label: "Scanner",
    searching: "Procurando…",
    noneFound: "Nenhum scanner encontrado",
    resolution: "Resolução",
    dpiWhenNotGrading: "300 dpi quando não há graduação.",
    sides: "Lados",
    duplex: "Frente + verso",
    frontOnly: "Só a frente",
    multifeed: "Detecção de alimentação múltipla",
    shared: "Estas são suas configurações de Scanner: mudá-las aqui muda para todas as sessões de scanner.",
  },
  identifyWith: "Identificar com",
  model: "Modelo",
  noModel: "Nenhum modelo baixado",
  notDownloaded: "não baixado",
  localNote:
    "Roda neste computador: lê o nome, o número de colecionador e o ano impressos em cada carta e estima notas e condição. Sem preços nem outros candidatos.",
  identifyAs: "Identificar como",
  grading: "Graduação",
  imagesPerCard: "Imagens por carta",
  conditionScale: "Escala de condição",
  priceStats: "Estatísticas de preço",
  readSlabs: "Ler etiquetas de slabs graduados",
  syncRemote: "Sincronizar com o Ximilar Cards Cloud",
  known: {
    title: "Mais rápido e mais preciso",
    intro:
      "Diga ao Ximilar o que você já sabe sobre as cartas, para ele não ter que descobrir. Marque só o que vale para todas as imagens: um valor errado piora as correspondências.",
    skipDetection: "Pular detecção",
    skipDetectionHint:
      "Para imagens já recortadas na carta: cada imagem é enviada como a própria carta, então o Ximilar não a procura na foto.",
    skipDetectionSlab: "As etiquetas de slab são achadas pela detecção, então podem não ser lidas.",
    everyCard: {
      tcg: "Todas são cartas colecionáveis",
      sport: "Todas são cartas esportivas",
      comics: "Todas são quadrinhos",
      slab: "Todas são slabs graduados",
    },
    cardType: "Tipo de carta",
    sendsCategory: "Envia a categoria “{category}”.",
    chooseTypeFirst: "Escolha um tipo em “Identificar como” primeiro; a Detecção automática deixa isso para o Ximilar.",
    frontSide: "Frente",
    frontSideHint: "As imagens mostram a frente (versos enviados com “frente e verso” são marcados como verso).",
    upright: "Na posição certa",
    uprightHint:
      "Nenhuma carta está girada ou de cabeça para baixo. Caso contrário, o Ximilar endireita cada carta antes de identificá-la.",
    game: "Jogo",
    alphabet: "Alfabeto",
    gameNote:
      "O jogo limita a busca às cartas dele; o alfabeto (ex. japonês para impressões japonesas) evita que as cartas correspondam à versão de outro idioma.",
    gameNoAlphabet:
      "Com um jogo definido, o Ximilar não lê mais o alfabeto e assume o latino: escolha-o para impressões em outras escritas.",
    sport: "Esporte",
    sportNote: "Limita a busca às cartas desse esporte.",
  },
  subcategory: {
    anyGame: "Qualquer jogo",
    anySport: "Qualquer esporte",
    readFromCard: "O Ximilar lê isso da carta",
  },
  alphabets: {
    detect: "Detectar",
    latin: "Latino",
    latinHint: "Inglês, idiomas europeus",
    japanese: "Japonês",
    chinese: "Chinês",
    korean: "Coreano",
    russian: "Russo",
    thai: "Tailandês",
  },
  more: {
    label: "Mais opções",
    summary: "— o que você sabe sobre as cartas, frente e verso, Magic AI, endpoint personalizado",
  },
  bothSides: {
    label: "Identificar pela frente e pelo verso",
    csv: "Usa a coluna de imagem do verso do CSV.",
    paired:
      "Imagens chamadas <code>card_F.jpg</code> e <code>card_B.jpg</code> (ou front/back) são pareadas pelo nome; caso contrário, imagens consecutivas.",
    why: "O verso vai para o reconhecimento junto com a frente — isso ajuda as cartas esportivas, cujo verso traz o jogador, o ano e o número.",
    frontOnly: "A identificação “{type}” lê só a frente: os versos são todos iguais.",
    magicAi: "Magic AI para cartas esportivas",
    magicAiHint:
      "Quando uma carta esportiva não está no banco de dados do Ximilar, um modelo de IA a identifica pelas imagens. Créditos extras de API só são usados quando ele roda.",
    magicAiSportOnly: "Vale só para cartas esportivas.",
    autoNote:
      "A Detecção automática identifica uma carta em uma chamada; uma carta esportiva é então enviada também ao Sports ID para estas opções (uma chamada a mais).",
    customEndpoint: "Endpoint personalizado (opcional)",
    customEndpointHint:
      "Um endpoint de reconhecimento que o Ximilar configurou para você. Ele recebe a mesma requisição que o padrão (frente e verso, se escolhido acima), e as respostas são lidas como “{type}”.",
    customEndpointHintAuto:
      "Um endpoint de reconhecimento que o Ximilar configurou para você. Ele recebe a mesma requisição que o padrão (frente e verso, se escolhido acima), e as respostas são lidas como o tipo de carta detectado.",
  },
  options: {
    dpiIdentification: "identificação",
    dpiGrading: "graduação",
    tradingCards: "cartas colecionáveis",
    sportsCards: "cartas esportivas",
    apiOnly: "só API",
    multifeedOffHint: "recomendado para cartas",
    multifeedDefault: "Padrão do driver",
    single: "Uma imagem por carta",
    frontBack: "Pares frente + verso",
  },
  openSettings: "Abrir Configurações",
  remote: {
    label: "Continuar uma sessão do Cards Cloud (opcional)",
    placeholder: "ID ou link da sessão",
    untitled: "Sem título",
    found: { one: "“{name}” · {count} carta", other: "“{name}” · {count} cartas" },
    paste: "Cole o ID da sessão ou o link dela do Ximilar App.",
    added: "{found}. As novas cartas são adicionadas a ela.",
    addedSkips:
      "{found}. As novas cartas são adicionadas a ela; imagens que ela já tem (comparadas pelo nome do arquivo) são ignoradas.",
    lookingUp: "Buscando…",
    hint: "Adiciona as cartas a uma sessão iniciada antes, ex. em outro computador.",
    hintSkips:
      "Adiciona as cartas a uma sessão iniciada antes, ex. em outro computador; imagens que ela já tem (comparadas pelo nome do arquivo) são ignoradas.",
  },
};
export default messages;
