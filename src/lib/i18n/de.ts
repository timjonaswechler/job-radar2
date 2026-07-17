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
    sidebarLabel: "Hauptnavigation",
    skipToMain: "Zum Hauptinhalt",
    groups: {
      jobRadar: "Job Radar",
    },
    items: {
      overview: "Übersicht",
      postings: "Stellenanzeigen",
      sources: "Quellen",
      searchRequests: "Search Requests",
      settings: "Einstellungen",
      postingsInbox: "Stellenanzeigen-Inbox",
      notFound: "Nicht gefunden",
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
  settings: {
    agents: {
      title: "Agent-Provider",
      description:
        "Verwalte Anmeldungen und API-Schlüssel. Das Modell wird später pro Agent Chat gewählt.",
      loading: "Provider-Status wird geladen…",
      modelCount_one: "{{count}} Modell",
      modelCount_other: "{{count}} Modelle",
      unavailableDescription:
        "Dieser Provider kann mit der aktuellen Konfiguration nicht verwendet werden.",
      status: {
        configured: "Konfiguriert",
        available: "Nicht konfiguriert",
        unavailable: "Nicht verfügbar",
      },
      actions: {
        reload: "Dateien neu laden",
        openFolder: "Agent-Datenordner öffnen",
        login: "Im Browser anmelden",
        replaceSubscription: "Anmeldung ersetzen",
        cancelLogin: "Anmeldung abbrechen",
        saveApiKey: "API-Schlüssel speichern",
        replaceApiKey: "API-Schlüssel ersetzen",
        logout: "Abmelden",
        removeApiKey: "API-Schlüssel entfernen",
        remove: "Entfernen",
        cancel: "Abbrechen",
      },
      subscription: {
        title: "Abonnement-Anmeldung",
        description:
          "Die Anmeldung wird im Browser fortgesetzt. Zugangsdaten werden nicht in dieser Ansicht angezeigt.",
      },
      apiKey: {
        title: "API-Schlüssel",
        description:
          "Der Schlüssel wird sicher übergeben und danach nicht mehr angezeigt.",
        replaceDescription:
          "Ein Schlüssel ist hinterlegt. Ein neuer Wert ersetzt den bisherigen Schlüssel.",
        required: "Gib einen API-Schlüssel ein.",
      },
      progress: {
        starting: "Anmeldung wird vorbereitet…",
        openingBrowser: "Browser wird geöffnet…",
        waitingForBrowser: "Warte auf die Anmeldung im Browser…",
        finalizing: "Anmeldung wird abgeschlossen…",
        completed: "Anmeldung abgeschlossen.",
        cancelled: "Anmeldung abgebrochen.",
        failed: "Anmeldung fehlgeschlagen.",
      },
      removeDialog: {
        title: "Authentifizierung entfernen?",
        description:
          "Die aktive Authentifizierung für {{provider}} wird entfernt. Andere Provider bleiben unverändert.",
      },
      notices: {
        title: "Agent-Einstellungen aktualisiert",
        reloaded: "Authentifizierung und Modelldateien wurden neu geladen.",
        loginComplete: "Die Abonnement-Anmeldung wurde gespeichert.",
        apiKeySaved: "Der API-Schlüssel wurde gespeichert und aus dem Eingabefeld entfernt.",
        authenticationRemoved: "Die Authentifizierung wurde entfernt.",
      },
      diagnostics: {
        title: "Konfiguration nicht vollständig verfügbar",
        authenticationInvalid:
          "Die Authentifizierungsdatei ist ungültig oder nicht lesbar. Gespeicherte Werte werden nicht angezeigt.",
        modelsInvalid:
          "Die Modelldatei ist ungültig. Die zuletzt gültige Modellliste bleibt aktiv.",
        unknown: "Eine Konfigurationsdatei ist ungültig oder nicht lesbar.",
      },
      errors: {
        title: "Agent-Einstellungen konnten nicht aktualisiert werden",
        unavailable:
          "Die Aktion ist derzeit nicht verfügbar. Prüfe die Konfigurationsdateien und versuche es erneut.",
        apiKey: "Der API-Schlüssel konnte nicht gespeichert werden.",
        login: "Die Browser-Anmeldung konnte nicht abgeschlossen werden.",
        cancelled: "Die Browser-Anmeldung wurde abgebrochen.",
      },
    },
  },
} as const satisfies TranslationTree;
