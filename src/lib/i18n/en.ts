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
    sidebarLabel: "Main navigation",
    skipToMain: "Skip to main content",
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
      notFound: "Not found",
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
  settings: {
    agents: {
      title: "Agent providers",
      description:
        "Manage sign-ins and API keys. Models are selected later for each Agent Chat.",
      loading: "Loading provider status…",
      modelCount_one: "{{count}} model",
      modelCount_other: "{{count}} models",
      unavailableDescription:
        "This provider cannot be used with the current configuration.",
      status: {
        configured: "Configured",
        available: "Not configured",
        unavailable: "Unavailable",
      },
      actions: {
        reload: "Reload files",
        openFolder: "Open Agent data folder",
        login: "Sign in in browser",
        replaceSubscription: "Replace sign-in",
        cancelLogin: "Cancel sign-in",
        saveApiKey: "Save API key",
        replaceApiKey: "Replace API key",
        logout: "Sign out",
        removeApiKey: "Remove API key",
        remove: "Remove",
        cancel: "Cancel",
      },
      subscription: {
        title: "Subscription sign-in",
        description:
          "Sign-in continues in the browser. Credentials are never displayed here.",
      },
      apiKey: {
        title: "API key",
        description:
          "The key is submitted securely and is not displayed afterward.",
        replaceDescription:
          "A key is configured. A new value replaces the existing key.",
        required: "Enter an API key.",
      },
      progress: {
        starting: "Preparing sign-in…",
        openingBrowser: "Opening browser…",
        waitingForBrowser: "Waiting for browser sign-in…",
        finalizing: "Finishing sign-in…",
        completed: "Sign-in complete.",
        cancelled: "Sign-in cancelled.",
        failed: "Sign-in failed.",
      },
      removeDialog: {
        title: "Remove authentication?",
        description:
          "The active authentication for {{provider}} will be removed. Other providers are unchanged.",
      },
      notices: {
        title: "Agent settings updated",
        reloaded: "Authentication and model files were reloaded.",
        loginComplete: "The subscription sign-in was saved.",
        apiKeySaved: "The API key was saved and cleared from the input.",
        authenticationRemoved: "The authentication was removed.",
      },
      diagnostics: {
        title: "Configuration is not fully available",
        authenticationInvalid:
          "The authentication file is invalid or unreadable. Stored values are not displayed.",
        modelsInvalid:
          "The model file is invalid. The last valid model list remains active.",
        unknown: "A configuration file is invalid or unreadable.",
      },
      errors: {
        title: "Agent settings could not be updated",
        unavailable:
          "This action is currently unavailable. Check the configuration files and try again.",
        apiKey: "The API key could not be saved.",
        login: "Browser sign-in could not be completed.",
        cancelled: "Browser sign-in was cancelled.",
      },
    },
  },
} as const satisfies TranslationShape<typeof de>;
