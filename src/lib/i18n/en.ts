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
      agentChatPrototype: "Agent Chat prototype",
      guidedSourceRepairPrototype: "Guided Source Repair prototype",
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
  agentChat: {
    loading: "Loading Agent Chat…",
    status: {
      saved: "Saved",
      running: "Active",
      modelUnavailable: "Model missing",
      readOnly: "Read-only",
      damaged: "Damaged",
      notSaved: "Not saved",
      waiting: "The Agent is responding…",
    },
    messages: {
      reasoning: "Reasoning",
      reasoningRedacted: "Reasoning is unavailable",
    },
    composer: {
      label: "Message the Agent",
      placeholder: "Message the Agent…",
    },
    actions: {
      send: "Send message",
      stop: "Stop response",
      resize: "Resize Chat and Canvas",
      selectModel: "Select Agent Model",
      selectReasoning: "Select Reasoning Level",
      compact: "Compact Agent Chat",
      reload: "Reload saved Chat",
    },
    context: {
      label: "Context usage",
      estimated:
        "Usage is an estimate of the provider context for the next request.",
      unavailable: "Context usage is unavailable",
      unavailableShort: "Context unavailable",
    },
    reasoning: {
      off: "Off",
      minimal: "Minimal",
      low: "Low",
      medium: "Medium",
      high: "High",
      x_high: "Very high",
      max: "Maximum",
    },
    compaction: {
      running: "Compacting the Agent Chat…",
      marker: "Context compacted",
      tokensBefore: "{{count}} tokens before",
    },
    notices: {
      aborted: "The response was stopped and not saved.",
      compacted: "The Agent Chat was compacted.",
      compactionCancelled: "Compaction was cancelled.",
    },
    recovery: {
      title: "Agent Chat was recovered",
      incompleteFinalTurnDiscarded:
        "An incomplete final response was discarded. The previously saved history remains available.",
    },
    states: {
      title: "Agent Chat is restricted",
      running:
        "The Agent Chat already has an active operation. It can be stopped.",
      modelUnavailable:
        "The previously used Agent Model is unavailable. Explicitly select an available model.",
      readOnlyLocked:
        "The Agent Chat is open in another process and can only be read here.",
      readOnlyUnsupported:
        "The Agent Chat contains a recognized but unsupported format and can only be read.",
      damaged:
        "The saved Agent Chat data is damaged. The Chat cannot be continued.",
    },
    errors: {
      loadTitle: "Agent Chat could not be opened",
      actionTitle: "Action failed",
      authenticationUnavailable:
        "The provider sign-in could not be used for this request. Reload Agent settings or sign in again.",
      invalidConfiguration:
        "The Agent configuration is invalid or unavailable.",
      rateLimited:
        "The provider rate limit has been reached. Try again later.",
      transportUnavailable:
        "The provider is currently unreachable over the network.",
      modelUnavailable:
        "The selected Agent Model is unavailable. Select another model.",
      contextFull:
        "The context is full. Compact the Chat or select a model with a larger context.",
      providerFailed:
        "The provider could not process the request.",
      chatBusy: "The Agent Chat already has an active operation.",
      readOnly: "The Agent Chat is read-only.",
      unsupported: "This Agent Chat format can only be read.",
      damaged: "The saved Agent Chat data is damaged.",
      notSaved: "The response was generated but could not be saved.",
      unavailable: "The Agent Chat is currently unavailable.",
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
      configuredOnlyDescription:
        "Configuration is present, but Job Radar has no executable provider and authentication combination for these models.",
      status: {
        executable: "Executable",
        configuredOnly: "Configured, not executable",
        catalogOnly: "Not configured",
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
        displayingDeviceCode: "Waiting for device-code sign-in…",
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
