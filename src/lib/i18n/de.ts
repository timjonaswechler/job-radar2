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
      agentChatPrototype: "Agent-Chat-Prototyp",
      guidedSourceRepairPrototype: "Guided-Source-Repair-Prototyp",
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
  agentChat: {
    loading: "Agent Chat wird geladen…",
    status: {
      saved: "Gespeichert",
      running: "Aktiv",
      modelUnavailable: "Modell fehlt",
      readOnly: "Schreibgeschützt",
      damaged: "Beschädigt",
      notSaved: "Nicht gespeichert",
      waiting: "Der Agent antwortet…",
    },
    messages: {
      reasoning: "Reasoning",
      reasoningRedacted: "Reasoning ist nicht verfügbar",
    },
    composer: {
      label: "Nachricht an den Agenten",
      placeholder: "Nachricht an den Agenten…",
    },
    actions: {
      send: "Nachricht senden",
      stop: "Antwort stoppen",
      resize: "Chat und Canvas in der Größe ändern",
      selectModel: "Agent Model auswählen",
      selectReasoning: "Reasoning Level auswählen",
      compact: "Agent Chat komprimieren",
      reload: "Gespeicherten Chat neu laden",
    },
    context: {
      label: "Kontextnutzung",
      estimated:
        "Die Nutzung ist eine Schätzung des Provider-Kontexts für die nächste Anfrage.",
      unavailable: "Kontextnutzung ist nicht verfügbar",
      unavailableShort: "Kontext nicht verfügbar",
    },
    reasoning: {
      off: "Aus",
      minimal: "Minimal",
      low: "Niedrig",
      medium: "Mittel",
      high: "Hoch",
      x_high: "Sehr hoch",
      max: "Maximal",
    },
    compaction: {
      running: "Der Agent Chat wird komprimiert…",
      marker: "Kontext komprimiert",
      tokensBefore: "{{count}} Tokens zuvor",
    },
    notices: {
      aborted: "Die Antwort wurde gestoppt und nicht gespeichert.",
      compacted: "Der Agent Chat wurde komprimiert.",
      compactionCancelled: "Die Komprimierung wurde abgebrochen.",
    },
    recovery: {
      title: "Agent Chat wurde wiederhergestellt",
      incompleteFinalTurnDiscarded:
        "Eine unvollständige letzte Antwort wurde verworfen. Der vorherige gespeicherte Verlauf ist weiterhin verfügbar.",
    },
    states: {
      title: "Agent Chat ist eingeschränkt",
      running:
        "Im Agent Chat läuft bereits eine Aktion. Sie kann gestoppt werden.",
      modelUnavailable:
        "Das zuletzt verwendete Agent Model ist nicht verfügbar. Wähle ausdrücklich ein verfügbares Modell aus.",
      readOnlyLocked:
        "Der Agent Chat ist in einem anderen Prozess geöffnet und kann hier nur gelesen werden.",
      readOnlyUnsupported:
        "Der Agent Chat enthält ein erkanntes, aber nicht unterstütztes Format und kann nur gelesen werden.",
      damaged:
        "Die gespeicherten Agent-Chat-Daten sind beschädigt. Der Chat kann nicht fortgesetzt werden.",
    },
    errors: {
      loadTitle: "Agent Chat konnte nicht geöffnet werden",
      actionTitle: "Aktion fehlgeschlagen",
      authenticationUnavailable:
        "Die Provider-Anmeldung konnte für diese Anfrage nicht verwendet werden. Lade die Agent-Einstellungen neu oder melde dich erneut an.",
      invalidConfiguration:
        "Die Agent-Konfiguration ist ungültig oder nicht verfügbar.",
      rateLimited:
        "Der Provider hat das Anfrage-Limit erreicht. Versuche es später erneut.",
      transportUnavailable:
        "Der Provider ist über das Netzwerk derzeit nicht erreichbar.",
      modelUnavailable:
        "Das ausgewählte Agent Model ist nicht verfügbar. Wähle ein anderes Modell.",
      contextFull:
        "Der Kontext ist voll. Komprimiere den Chat oder wähle ein Modell mit größerem Kontext.",
      providerFailed:
        "Der Provider konnte die Anfrage nicht verarbeiten.",
      chatBusy: "Im Agent Chat läuft bereits eine Aktion.",
      readOnly: "Der Agent Chat ist schreibgeschützt.",
      unsupported: "Dieses Agent-Chat-Format kann nur gelesen werden.",
      damaged: "Die gespeicherten Agent-Chat-Daten sind beschädigt.",
      notSaved: "Die Antwort wurde erzeugt, konnte aber nicht gespeichert werden.",
      unavailable: "Der Agent Chat ist derzeit nicht verfügbar.",
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
      configuredOnlyDescription:
        "Die Konfiguration ist vorhanden, aber Job Radar hat keine ausführbare Kombination aus Provider und Authentifizierung für diese Modelle.",
      status: {
        executable: "Ausführbar",
        configuredOnly: "Konfiguriert, nicht ausführbar",
        catalogOnly: "Nicht konfiguriert",
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
        displayingDeviceCode: "Warte auf die Gerätecode-Anmeldung…",
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
