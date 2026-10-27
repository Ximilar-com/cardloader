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
      "The model is made by Liquid AI and downloaded from Hugging Face; it is not part of cardloader. Its license allows commercial use only below a revenue threshold — read it before accepting.",
    resume: "Resume download",
    download: "Download",
    partial: "{done} of {size} already downloaded",
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
