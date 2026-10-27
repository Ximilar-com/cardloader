import type { Messages } from "..";
import type en from "../en/newSession";

const messages: Messages<typeof en> = {
  title: {
    new: "Nueva sesión",
    edit: "Ajustes de la sesión",
    duplicate: "Duplicar sesión",
  },
  copyName: "{name} (copia)",
  importing: "Importando…",
  import: "Importar",
  create: "Crear",
  importAndRun: "Importar y ejecutar",
  importAndRunTitle: "Importar, luego analizar las imágenes y subir según lo indicado abajo",
  createAndRun: "Crear y ejecutar",
  needsToken: "Configura tu token de API de Ximilar en Ajustes antes de ejecutar una sesión.",
  needsModel: "Descarga un modelo local en Ajustes antes de ejecutar esta sesión.",
  editNote:
    "Los cambios se aplican a las cartas que se analicen a partir de ahora. Las cartas ya analizadas conservan sus resultados; usa <b>Duplicar</b> para analizar de nuevo el mismo origen con otros ajustes.",
  name: "Nombre",
  source: "Origen",
  sources: {
    folder: "Analizar las imágenes de una carpeta",
    watch_folder: "Analizar imágenes a medida que otra app las guarda",
    scanner: "Escanear un montón de cartas (p. ej. fi-8170)",
    csv: "Importar cartas o rutas de imágenes desde una hoja de cálculo",
  },
  browse: "Examinar",
  csv: {
    label: "Archivo CSV",
    help: "Una carta por fila. Las filas con nombre de carta (y set, número, precio, nota…) se importan como cartas identificadas; las filas que solo tienen la ruta de una imagen se analizan. Una exportación CSV de cardloader se vuelve a importar tal como era.",
    reanalyze: "Analizar de nuevo las imágenes de las {count} filas identificadas",
    summary: "<b>{rows}</b>: {cards} para importar, {images} para analizar.",
    summarySkipped: "<b>{rows}</b>: {cards} para importar, {images} para analizar, {skipped} vacías omitidas.",
    rows: { one: "{count} fila", other: "{count} filas" },
    identifiedCards: { one: "{count} carta identificada", other: "{count} cartas identificadas" },
    images: { one: "{count} imagen", other: "{count} imágenes" },
    missingImages: {
      one: "{count} fila apunta a imágenes que no existen.",
      other: "{count} filas apuntan a imágenes que no existen.",
    },
    columns: "Columnas: {columns}",
  },
  folder: {
    label: "Carpeta",
    chooseWatch: "Elige la carpeta en la que tu app de escaneo guarda las imágenes.",
    choose: "Elige la carpeta con las imágenes de tus cartas.",
    noImages: "No hay imágenes en esta carpeta (JPEG, PNG, WebP, TIFF o BMP).",
    found: { one: "{count} imagen encontrada", other: "{count} imágenes encontradas" },
    foundWatching: {
      one: "{count} imagen encontrada: las imágenes nuevas se recogerán mientras la sesión esté en curso",
      other: "{count} imágenes encontradas: las imágenes nuevas se recogerán mientras la sesión esté en curso",
    },
  },
  cost: {
    local: "identificadas en este ordenador, sin llamadas a la API",
    calls: { one: "aprox. {count} llamada a la API", other: "aprox. {count} llamadas a la API" },
    callsCloud: {
      one: "aprox. {count} llamada a la API para Cards Cloud",
      other: "aprox. {count} llamadas a la API para Cards Cloud",
    },
    callsMagicAi: {
      one: "aprox. {count} llamada a la API (Magic AI puede añadir más)",
      other: "aprox. {count} llamadas a la API (Magic AI puede añadir más)",
    },
  },
  scanner: {
    label: "Escáner",
    searching: "Buscando…",
    noneFound: "No se encontró ningún escáner",
    resolution: "Resolución",
    dpiWhenNotGrading: "300 dpi si no se grada.",
    sides: "Caras",
    duplex: "Anverso + reverso",
    frontOnly: "Solo anverso",
    multifeed: "Detección de alimentación múltiple",
    shared: "Estos son tus ajustes de Escáner: si los cambias aquí, cambian para todas las sesiones de escáner.",
  },
  identifyWith: "Identificar con",
  model: "Modelo",
  noModel: "Ningún modelo descargado",
  notDownloaded: "sin descargar",
  localNote:
    "Funciona en este ordenador: lee el nombre, el número de colección y el año impresos en cada carta, y estima notas y estado. Sin precios ni otros candidatos.",
  identifyAs: "Identificar como",
  grading: "Gradación",
  imagesPerCard: "Imágenes por carta",
  conditionScale: "Escala de estado",
  priceStats: "Estadísticas de precios",
  readSlabs: "Leer etiquetas de slabs gradados",
  syncRemote: "Sincronizar con Ximilar Cards Cloud",
  known: {
    title: "Más rápido y más preciso",
    intro:
      "Dile a Ximilar lo que ya sabes de las cartas para que no tenga que averiguarlo. Marca solo lo que sea cierto en todas las imágenes: un valor incorrecto empeora las coincidencias.",
    skipDetection: "Omitir la detección",
    skipDetectionHint:
      "Para imágenes ya recortadas a la carta: cada imagen se envía como la carta en sí, así Ximilar no la busca dentro de la foto.",
    skipDetectionSlab: "Las etiquetas de los slabs se encuentran mediante la detección, así que puede que no se lean.",
    everyCard: {
      tcg: "Todas son cartas coleccionables",
      sport: "Todas son cartas deportivas",
      comics: "Todos son cómics",
      slab: "Todas son slabs gradados",
    },
    cardType: "Tipo de carta",
    sendsCategory: "Envía la categoría «{category}».",
    chooseTypeFirst: "Primero elige un tipo en «Identificar como»; con Detección automática lo decide Ximilar.",
    frontSide: "Anverso",
    frontSideHint:
      "Las imágenes muestran el anverso (los reversos enviados con «anverso y reverso» se marcan como reverso).",
    upright: "En posición correcta",
    uprightHint: "Ninguna carta está girada ni boca abajo. Si no, Ximilar endereza cada carta antes de identificarla.",
    game: "Juego",
    alphabet: "Alfabeto",
    gameNote:
      "El juego limita la búsqueda a sus cartas; el alfabeto (p. ej. japonés para ediciones japonesas) evita que las cartas coincidan con la versión en otro idioma.",
    gameNoAlphabet:
      "Con un juego elegido, Ximilar ya no lee el alfabeto y asume el latino: elígelo para ediciones en otras escrituras.",
    sport: "Deporte",
    sportNote: "Limita la búsqueda a las cartas de ese deporte.",
  },
  subcategory: {
    anyGame: "Cualquier juego",
    anySport: "Cualquier deporte",
    readFromCard: "Ximilar lo lee de la carta",
  },
  alphabets: {
    detect: "Detectar",
    latin: "Latino",
    latinHint: "Inglés, idiomas europeos",
    japanese: "Japonés",
    chinese: "Chino",
    korean: "Coreano",
    russian: "Ruso",
    thai: "Tailandés",
  },
  more: {
    label: "Más opciones",
    summary: "— lo que sabes de las cartas, anverso y reverso, Magic AI, endpoint personalizado",
  },
  bothSides: {
    label: "Identificar por el anverso y el reverso",
    csv: "Usa la columna de imagen del reverso del CSV.",
    paired:
      "Las imágenes llamadas <code>card_F.jpg</code> y <code>card_B.jpg</code> (o front/back) se emparejan por nombre; si no, por orden consecutivo.",
    why: "El reverso se envía a la identificación junto con su anverso: esto ayuda con las cartas deportivas, cuyo reverso muestra el jugador, el año y el número.",
    frontOnly: "Con «{type}», la identificación lee solo el anverso: todos sus reversos son iguales.",
    magicAi: "Magic AI para cartas deportivas",
    magicAiHint:
      "Cuando una carta deportiva no está en la base de datos de Ximilar, un modelo de IA la identifica a partir de sus imágenes. Solo se gastan créditos de API extra cuando se usa.",
    magicAiSportOnly: "Solo se aplica a cartas deportivas.",
    autoNote:
      "La detección automática identifica una carta en una llamada; para estas opciones, una carta deportiva se envía además a Sports ID (una llamada más).",
    customEndpoint: "Endpoint personalizado (opcional)",
    customEndpointHint:
      "Un endpoint de identificación que Ximilar configuró para ti. Recibe la misma petición que el estándar (anverso y reverso si lo elegiste arriba) y sus respuestas se leen como «{type}».",
    customEndpointHintAuto:
      "Un endpoint de identificación que Ximilar configuró para ti. Recibe la misma petición que el estándar (anverso y reverso si lo elegiste arriba) y sus respuestas se leen como el tipo de carta detectado.",
  },
  options: {
    dpiIdentification: "identificación",
    dpiGrading: "gradación",
    tradingCards: "cartas coleccionables",
    sportsCards: "cartas deportivas",
    apiOnly: "solo API",
    multifeedOffHint: "recomendado para cartas",
    multifeedDefault: "Por defecto del driver",
    single: "Una imagen por carta",
    frontBack: "Pares anverso + reverso",
  },
  openSettings: "Abrir Ajustes",
  remote: {
    label: "Continuar una sesión de Cards Cloud (opcional)",
    placeholder: "ID o enlace de la sesión",
    untitled: "Sin título",
    found: { one: "«{name}» · {count} carta", other: "«{name}» · {count} cartas" },
    paste: "Pega el ID de la sesión o su enlace desde la Ximilar App.",
    added: "{found}. Las cartas nuevas se añaden a ella.",
    addedSkips:
      "{found}. Las cartas nuevas se añaden a ella; las imágenes que ya tiene (según el nombre de archivo) se omiten.",
    lookingUp: "Buscándola…",
    hint: "Añade las cartas a una sesión empezada antes, p. ej. en otro ordenador.",
    hintSkips:
      "Añade las cartas a una sesión empezada antes, p. ej. en otro ordenador; las imágenes que ya tiene (según el nombre de archivo) se omiten.",
  },
};
export default messages;
