import type { TranslationTree } from "./types";

export const de = {
  common: {
    actions: {
      search: "Suchen",
    },
    status: {
      soon: "Bald",
    },
    empty: {
      noResults: "Keine Ergebnisse gefunden.",
    },
  },
  language: {
    actions: {
      select: "Sprache auswählen",
    },
  },
  theme: {
    actions: {
      switchToDark: "Zum dunklen Modus wechseln",
      switchToLight: "Zum hellen Modus wechseln",
    },
  },
  navigation: {
    groups: {
      jobRadar: "Job Radar",
    },
    items: {
      overview: "Übersicht",
      postings: "Stellenanzeigen",
      sources: "Quellen",
      settings: "Einstellungen",
      postingsInbox: "Stellenanzeigen-Inbox",
    },
  },
  commandSearch: {
    input: {
      placeholder: "Seiten öffnen…",
    },
    groups: {
      pages: "Seiten",
    },
  },
  startup: {
    checking: "Startcheck läuft…",
    browserMode: {
      title: "Browser-Modus",
      description: "Tauri und SQLite prüfen wir nur in der Desktop-App.",
    },
    ready: {
      title: "Alles bereit",
      description:
        "Tauri ist verbunden, SQLite {{sqliteVersion}} ist erreichbar.",
    },
    failed: {
      title: "Startcheck fehlgeschlagen",
    },
  },
  features: {
    applications: {
      actions: {
        new: "Neue Bewerbung",
      },
    },
  },
} as const satisfies TranslationTree;
