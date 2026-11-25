import type { Messages } from "..";
import type en from "../en/wizard";

const messages: Messages<typeof en> = {
  title: "Configurar Cardloader",
  progress: "Paso {step} de {steps} · puedes cambiarlo todo después en Ajustes",
  alwaysIncluded: "Siempre incluido",
  language: {
    title: "Elige tu idioma",
    text: "Cardloader lo usará a partir de ahora.",
  },
  interests: {
    title: "¿Qué vas a escanear?",
    text: "Elige todo lo que coleccionas.",
    tcg: "Juegos de cartas coleccionables",
    tcgHint: "Pokémon, Magic, Yu-Gi-Oh!, One Piece, Lorcana…",
    sport: "Cartas deportivas",
    sportHint: "Béisbol, baloncesto, fútbol americano, hockey, fútbol…",
    comics: "Cómics",
    comicsHint: "Cómics, revistas y manga",
  },
  games: {
    title: "¿Qué juegos?",
    text: "Elige uno y cada sesión nueva se configura con él, así Ximilar solo busca coincidencias de ese juego. Elige varios, o ninguno, para decidirlo en cada sesión.",
  },
  goals: {
    title: "¿Qué quieres saber?",
    text: "Elige todo lo que necesites.",
    identification: "Identificación",
    identificationHint: "Nombre, set y número de cada carta",
    grading: "Gradación",
    gradingHint: "Una nota completa: esquinas, bordes, superficie y centrado",
    condition: "Estado",
    conditionHint: "Un estado rápido, p. ej. Near Mint, para anuncios",
    prices: "Precios",
    pricesHint: "Precios de mercado de cada carta",
    crops: "Cartas recortadas",
    cropsHint: "Cada carta recortada y enderezada, guardada en una carpeta",
  },
  sources: {
    title: "¿Cómo vas a añadir las cartas?",
    text: "Elige todo lo que uses. La primera opción se abre con Nueva sesión.",
    scanner: "Escáner",
    scannerHint: "Un escáner de documentos con alimentador, p. ej. RICOH fi-8170",
    folder: "Carpeta de imágenes",
    folderHint: "Fotos o escaneos que ya están en este ordenador",
    watchFolder: "Carpeta vigilada",
    watchFolderHint: "Las cartas se analizan a medida que llegan imágenes nuevas a una carpeta",
    csv: "Archivo CSV",
    csvHint: "Una lista de cartas o rutas de imágenes",
  },
  engine: {
    title: "¿Qué debe identificar las cartas?",
    ximilar: "API de Ximilar",
    ximilarHint: "Identificación precisa, precios y gradación. Necesita un token de API e internet.",
    local: "Modelo de IA local",
    localHint:
      "Funciona en este ordenador, sin conexión y gratis. Lee el nombre, el número y el set, y estima el estado; es menos preciso.",
    token: "Token de API",
    tokenSaved: "Ya está guardado; déjalo vacío para conservarlo",
    tokenPaste: "Pega tu token",
    tokenHelp: "Lo encontrarás en la <link>Ximilar App</link>, en tu perfil. Puedes añadirlo después en Ajustes.",
    pricesNeedApi:
      "Los precios de Ximilar solo llegan con su identificación: con el modelo local, toma los precios de los anuncios de eBay (tus claves de eBay en Ajustes).",
    roughGrades:
      "Las notas y estados del modelo local son estimaciones aproximadas de un modelo pequeño, no las de un evaluador.",
    cropsNeedApi:
      "Las imágenes recortadas las hace la API de Ximilar (añade tu token en Ajustes), salvo que elijas el modelo local en la pestaña Recortar de Nueva sesión.",
    gradeTitle: "¿Qué debe gradar las cartas?",
  },
  task: {
    title: "¿Qué debe hacer Cardloader con tus cartas?",
    text: "Así empiezan las sesiones nuevas; puedes elegir de nuevo en cada sesión.",
    identify: "Identificarlas",
    identifyHint: "Nombre, set, número y precio de cada carta, y un grado si quieres",
    grade: "Solo gradarlas",
    gradeHint: "Un grado completo de cada carta (esquinas, bordes, superficie, centrado), sin identificarla",
    condition: "Solo saber su estado",
    conditionHint: "El estado de cada carta, p. ej. Near Mint, para anuncios, sin identificarla",
    crop: "Solo recortarlas",
    cropHint: "Cada carta recortada de su imagen y enderezada, guardada como imagen en una carpeta",
  },
  token: {
    title: "Conecta la API de Ximilar",
    text: "El recorte se hace con la API de Ximilar: necesita un token de API e internet.",
  },
};
export default messages;
