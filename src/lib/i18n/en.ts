import type { de } from "./de";
import type { TranslationShape } from "./types";

export const en = {
  common: {
    actions: {
      search: "Search",
    },
    status: {
      soon: "Soon",
    },
    empty: {
      noResults: "No results found.",
    },
  },
  language: {
    actions: {
      select: "Select language",
    },
  },
  theme: {
    actions: {
      switchToDark: "Switch to dark mode",
      switchToLight: "Switch to light mode",
    },
  },
  navigation: {
    groups: {
      jobRadar: "Job Radar",
    },
    items: {
      overview: "Overview",
      postings: "Postings",
      sources: "Sources",
      searchRequests: "Search Requests",
      settings: "Settings",
      postingsInbox: "Postings Inbox",
    },
  },
  commandSearch: {
    input: {
      placeholder: "Open pages…",
    },
    groups: {
      pages: "Pages",
    },
  },
  startup: {
    checking: "Running startup check…",
    browserMode: {
      title: "Browser mode",
      description: "Tauri and SQLite are checked only in the desktop app.",
    },
    ready: {
      title: "Everything ready",
      description: "Tauri is connected, SQLite {{sqliteVersion}} is available.",
    },
    failed: {
      title: "Startup check failed",
    },
  },
  features: {
    applications: {
      actions: {
        new: "New Application",
      },
    },
  },
} as const satisfies TranslationShape<typeof de>;
