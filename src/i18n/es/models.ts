import type { Messages } from "..";
import type en from "../en/models";

const messages: Messages<typeof en> = {
  picker: {
    model: "Modelo",
    downloaded: "Descargado",
  },
  card: {
    sizes: "{size} de descarga · ~{memory} de memoria",
    modelPage: "Página del modelo",
    unsupported: "Esta versión de cardloader se compiló sin soporte para modelos locales.",
    progress: "{done} de {size} ({percent}%)",
    left: "Quedan ~{time}",
    ready: "Descargado · funciona en este ordenador, sin internet",
    accept: "Acepto la <license>{license}</license>",
    licenseNote:
      "El modelo lo crea {maker} y se descarga de Hugging Face; no forma parte de cardloader. Lee su licencia antes de aceptarla.",
    licenseLimit: "Solo permite el uso comercial por debajo de un umbral de ingresos.",
    resume: "Reanudar la descarga",
    download: "Descargar",
    partial: "{done} de {size} ya descargados",
  },
  device: {
    runOn: "Ejecutar en",
    auto: "Automático",
    noGpu: "no hay GPU, así que la CPU",
    cpu: "Solo la CPU",
    integrated: "{backend} · integrada",
    missing: "{name} (no encontrada: se usa la CPU)",
    tooSmall:
      "{model} necesita unos {need} y {gpu} tiene {have}: lo que no cabe se ejecuta en la CPU, que es más lenta.",
    loaded: "El modelo cargado se ejecuta en {runsOn}.",
    partly:
      "El modelo cargado ejecuta {gpu} de sus {layers} capas en {runsOn} y el resto en la CPU: la memoria libre de la GPU no alcanzaba para todo.",
    fellBack: "La GPU no pudo cargar el modelo, así que se ejecuta en la CPU ({error}).",
  },
  compare: {
    title: "Comparar con un modelo local",
    stop: "Detener",
    intro:
      "Un modelo local lee cada carta que Ximilar identificó en «{name}» y se comparan las dos respuestas. No cambia nada en la sesión. Pruébalo con varios modelos para ver cuál se adapta mejor a tus cartas.",
    downloadFirst: "Primero descarga un modelo local en Ajustes.",
    model: "Modelo",
    compareAgain: "Comparar de nuevo",
    compare: "Comparar",
    sameName: "Mismo nombre",
    sameNumber: "Mismo número",
    perCard: "Por carta",
    loadingModel: "Cargando el modelo…",
    progress: { one: "{done} de {count} carta", other: "{done} de {count} cartas" },
    of: "{n} de {total}",
    ofWithNumber: "{n} de {total} con número",
    onThisComputer: "en este ordenador",
    localModel: "Modelo local",
    name: "Nombre",
    number: "N.º",
    nothingRead: "no leyó nada",
    noCards: "Esta sesión no tiene cartas identificadas por Ximilar con las que comparar.",
    same: "igual",
    different: "distinto",
  },
};
export default messages;
