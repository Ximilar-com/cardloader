import type { Messages } from "..";
import type en from "../en/cards";

const messages: Messages<typeof en> = {
  title: "Cartas",
  total: { one: "<b>{count}</b> carta", other: "<b>{count}</b> cartas" },
  intro:
    "Todas las cartas identificadas de tus sesiones en este ordenador. Busca por nombre, set, número, ubicación o notas.",
  filter: {
    all: "Todas",
    review: "Por revisar",
    graded: "Gradadas",
    duplicates: "Duplicadas",
    agent: "Por IA",
  },
  selected: "{count} seleccionadas",
  accept: "Aceptar",
  accepted: { one: "{count} coincidencia aceptada", other: "{count} coincidencias aceptadas" },
  unaccept: "Desaceptar",
  unacceptTitle: "Deshacer las aceptaciones",
  unaccepted: { one: "{count} coincidencia desaceptada", other: "{count} coincidencias desaceptadas" },
  setLocation: "Asignar ubicación…",
  turnUpright: "Enderezar",
  turnUprightTitle: "Endereza las cartas que Ximilar encontró boca abajo, anverso y reverso",
  clearSelection: "Borrar la selección",
  searchPlaceholder: "Nombre, set, número, ubicación…",
  search: "Buscar cartas",
  showingNewest: "Se muestran las {count} más recientes: busca o filtra para encontrar las demás.",
  noMatch: "Ninguna carta coincide.",
  empty: "Aún no hay cartas identificadas.",
  goToSessions: "Ir a las sesiones",
  position: "Carta {at} de {count}",
  turned: "Enderezada",
  inSession: "En la sesión «{name}»",
  location: {
    title: { one: "Ubicación de {count} carta", other: "Ubicación de {count} cartas" },
    hint: "Dónde se guardan, p. ej. «Caja 4» o «Carpeta 2, página 7». Se puede buscar y se incluye en la exportación CSV.",
    confirm: "Guardar ubicación",
    done: { one: "{count} carta ahora en {location}", other: "{count} cartas ahora en {location}" },
  },
};
export default messages;
