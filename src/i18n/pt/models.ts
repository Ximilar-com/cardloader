import type { Messages } from "..";
import type en from "../en/models";

const messages: Messages<typeof en> = {
  picker: {
    model: "Modelo",
    downloaded: "Baixado",
  },
  card: {
    sizes: "Download de {size} · ~{memory} de memória",
    modelPage: "Página do modelo",
    unsupported: "Esta versão do cardloader foi compilada sem suporte a modelos locais.",
    progress: "{done} de {size} ({percent}%)",
    left: "faltam ~{time}",
    ready: "Baixado · roda neste computador, sem precisar de internet",
    accept: "Aceito a <license>{license}</license>",
    licenseNote:
      "O modelo é feito por {maker} e baixado do Hugging Face; ele não faz parte do cardloader. Leia a licença antes de aceitar.",
    licenseLimit: "Ela só permite uso comercial abaixo de um limite de receita.",
    resume: "Retomar download",
    download: "Baixar",
    partial: "{done} de {size} já baixados",
  },
  device: {
    runOn: "Executar em",
    auto: "Automático",
    noGpu: "nenhuma GPU encontrada, então a CPU",
    cpu: "Só a CPU",
    integrated: "{backend} · integrada",
    missing: "{name} (não encontrada: a CPU é usada)",
    tooSmall: "{model} precisa de cerca de {need} e {gpu} tem {have}: o que não couber roda na CPU, que é mais lenta.",
    loaded: "O modelo carregado roda em {runsOn}.",
    partly:
      "O modelo carregado roda {gpu} de suas {layers} camadas em {runsOn} e o resto na CPU: a memória livre da GPU não bastava.",
    fellBack: "A GPU não conseguiu carregar o modelo, então ele roda na CPU ({error}).",
  },
  compare: {
    title: "Comparar com um modelo local",
    stop: "Parar",
    intro:
      "Um modelo local lê cada carta que o Ximilar identificou em “{name}”, e as duas respostas são comparadas. Nada na sessão muda. Rode com vários modelos para ver qual combina melhor com suas cartas.",
    downloadFirst: "Baixe um modelo local em Configurações primeiro.",
    model: "Modelo",
    compareAgain: "Comparar de novo",
    compare: "Comparar",
    sameName: "Mesmo nome",
    sameNumber: "Mesmo número",
    perCard: "Por carta",
    loadingModel: "Carregando o modelo…",
    progress: { one: "{done} de {count} carta", other: "{done} de {count} cartas" },
    of: "{n} de {total}",
    ofWithNumber: "{n} de {total} com número",
    onThisComputer: "neste computador",
    localModel: "Modelo local",
    name: "Nome",
    number: "Nº",
    nothingRead: "nada lido",
    noCards: "Esta sessão não tem cartas identificadas pelo Ximilar para comparar.",
    same: "igual",
    different: "diferente",
  },
};
export default messages;
