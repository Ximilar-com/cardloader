import type { Messages } from "..";
import type en from "../en/wizard";

const messages: Messages<typeof en> = {
  title: "Configura Cardloader",
  progress: "Passo {step} di {steps} · puoi cambiare tutto più tardi in Impostazioni",
  alwaysIncluded: "Sempre incluso",
  language: {
    title: "Scegli la lingua",
    text: "Da ora Cardloader parlerà questa lingua.",
  },
  interests: {
    title: "Cosa scansionerai?",
    text: "Scegli tutto ciò che collezioni.",
    tcg: "Giochi di carte collezionabili",
    tcgHint: "Pokémon, Magic, Yu-Gi-Oh!, One Piece, Lorcana…",
    sport: "Carte sportive",
    sportHint: "Baseball, basket, football americano, hockey, calcio…",
  },
  games: {
    title: "Quali giochi?",
    text: "Scegline uno e ogni nuova sessione lo userà, così Ximilar cerca solo le carte di quel gioco. Scegline più di uno, o nessuno, per decidere a ogni sessione.",
  },
  goals: {
    title: "Cosa vuoi sapere?",
    text: "Scegli tutto ciò che ti serve.",
    identification: "Identificazione",
    identificationHint: "Nome, set e numero di ogni carta",
    grading: "Gradazione",
    gradingHint: "Un voto completo: angoli, bordi, superficie e centratura",
    condition: "Condizione",
    conditionHint: "Una condizione rapida, ad es. Near Mint, per le inserzioni",
    prices: "Prezzi",
    pricesHint: "Prezzi di mercato di ogni carta",
  },
  sources: {
    title: "Da dove arriveranno le carte?",
    text: "Scegli tutte quelle che usi. La prima si apre con Nuova sessione.",
    scanner: "Scanner",
    scannerHint: "Uno scanner documenti con alimentatore, ad es. RICOH fi-8170",
    folder: "Cartella di immagini",
    folderHint: "Foto o scansioni già su questo computer",
    watchFolder: "Cartella monitorata",
    watchFolderHint: "Le carte vengono analizzate man mano che arrivano nuove immagini in una cartella",
    csv: "File CSV",
    csvHint: "Un elenco di carte o di percorsi di immagini",
  },
  engine: {
    title: "Cosa deve identificare le carte?",
    ximilar: "Ximilar API",
    ximilarHint: "Identificazione precisa, prezzi e gradazione. Richiede un token API e internet.",
    local: "Modello IA locale",
    localHint:
      "Gira su questo computer, offline e gratis. Legge nome, numero e set e stima la condizione; meno preciso.",
    token: "Token API",
    tokenSaved: "Già salvato, lascia vuoto per mantenerlo",
    tokenPaste: "Incolla il tuo token",
    tokenHelp:
      "Lo trovi nella <link>Ximilar App</link> sotto il tuo profilo. Puoi aggiungerlo più tardi in Impostazioni.",
    pricesNeedApi: "I prezzi richiedono la Ximilar API.",
    roughGrades:
      "Voti e condizioni del modello locale sono stime approssimative di un piccolo modello, non di un esperto di gradazione.",
  },
};
export default messages;
