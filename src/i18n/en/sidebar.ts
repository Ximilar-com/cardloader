// sidebar: the app shell (navigation, toasts)

export default {
  nav: {
    home: "Home",
    sessions: "Sessions",
    cards: "Cards",
    settings: "Settings",
  },
  unsavedSettings: "Unsaved settings",
  poweredBy: "Powered by Ximilar",
  update: "Update",
  updateAvailable: "Cardloader {version} is available",
  toast: {
    noneUpsideDown: "None of these cards is upside down",
    turned: { one: "Turned {count} card upright", other: "Turned {count} cards upright" },
    turnedSkipped: {
      one: "Turned {count} card upright · {skipped} weren’t upside down",
      other: "Turned {count} cards upright · {skipped} weren’t upside down",
    },
  },
};
