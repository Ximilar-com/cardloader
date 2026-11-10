// models: on-device models (LocalModels) and "Compare with a local model" (CompareDialog). Model names,
// descriptions and tiers come from Rust and stay as they are.

export default {
  picker: {
    model: "Model",
    downloaded: "Downloaded",
  },
  card: {
    sizes: "{size} download · ~{memory} memory",
    modelPage: "Model page",
    unsupported: "This build of cardloader was made without local model support.",
    progress: "{done} of {size} ({percent}%)",
    left: "~{time} left",
    ready: "Downloaded · runs on this computer, no internet needed",
    accept: "I accept the <license>{license}</license>",
    licenseNote:
      "The model is made by {maker} and downloaded from Hugging Face; it is not part of cardloader. Read its license before accepting.",
    licenseLimit: "Its license allows commercial use only below a revenue threshold.",
    resume: "Resume download",
    download: "Download",
    partial: "{done} of {size} already downloaded",
  },
  device: {
    runOn: "Run on",
    auto: "Automatic",
    noGpu: "no GPU found, so the CPU",
    cpu: "CPU only",
    integrated: "{backend} · integrated",
    missing: "{name} (not found: the CPU is used)",
    tooSmall:
      "{model} needs about {need} and {gpu} has {have}: the part that doesn't fit runs on the CPU, which is slower.",
    loaded: "The loaded model runs on {runsOn}.",
    partly:
      "The loaded model runs {gpu} of its {layers} layers on {runsOn} and the rest on the CPU: the GPU's free memory couldn't hold it all.",
    fellBack: "The GPU couldn't load the model, so it runs on the CPU ({error}).",
  },
  compare: {
    title: "Compare with a local model",
    stop: "Stop",
    intro:
      "A local model reads each card Ximilar identified in “{name}”, and the two answers are compared. Nothing in the session changes. Run it with several models to see which suits your cards.",
    downloadFirst: "Download a local model in Settings first.",
    model: "Model",
    compareAgain: "Compare again",
    compare: "Compare",
    sameName: "Same name",
    sameNumber: "Same number",
    perCard: "Per card",
    loadingModel: "Loading the model…",
    progress: { one: "{done} of {count} card", other: "{done} of {count} cards" },
    of: "{n} of {total}",
    ofWithNumber: "{n} of {total} with a number",
    onThisComputer: "on this computer",
    localModel: "Local model",
    name: "Name",
    number: "No.",
    nothingRead: "nothing read",
    noCards: "This session has no cards identified by Ximilar to compare with.",
    same: "same",
    different: "different",
  },
};
