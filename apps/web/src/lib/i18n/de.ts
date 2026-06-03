export const de = {
  app: {
    title: "Job Radar",
    description:
      "Lokale Desktop-App zum Verwalten von Stellenanzeigen, Bewerbungen, Suchläufen und Erinnerungen.",
    openGithubRepository: "GitHub-Repository öffnen",
  },
  language: {
    selectLanguage: "Sprache auswählen",
    switchToEnglish: "Auf Englisch wechseln",
    switchToGerman: "Auf Deutsch wechseln",
  },
  navigation: {
    newApplication: "Neue Bewerbung",
    groups: {
      jobRadar: "Job Radar",
      search: "Suche",
      system: "System",
      other: "Weitere",
    },
    items: {
      overview: "Übersicht",
      mails: "Mails",
      postingsInbox: "Stellenanzeigen-Inbox",
      applications: "Bewerbungen",
      reminders: "Erinnerungen",
      searchQueries: "Suchanfragen",
      jobSources: "Jobquellen",
      searchRuns: "Suchläufe",
      data: "Import & Export",
      history: "Historie",
      settings: "Einstellungen",
    },
  },
  commandSearch: {
    button: "Suchen",
    placeholder: "Seiten, Abschnitte und Aktionen öffnen…",
    noResults: "Keine Ergebnisse gefunden.",
    soon: "Bald",
    groups: {
      pages: "Seiten",
      sections: "Abschnitte",
      actions: "Aktionen",
    },
    sections: {
      activeApplications: "Aktive Bewerbungen",
      language: "Sprache",
      import: "Import",
    },
    actions: {
      newApplication: "Neue Bewerbung",
      startSearchRun: "Suchlauf starten",
      newSearchQuery: "Neue Suchanfrage",
    },
  },
  search: {
    button: "Suchen",
    placeholder: "Stellenanzeigen, Bewerbungen und Suche öffnen…",
    noResults: "Keine Ergebnisse gefunden.",
    soon: "Bald",
  },
  theme: {
    switchToDark: "Zum dunklen Modus wechseln",
    switchToLight: "Zum hellen Modus wechseln",
  },
  applicationStatus: {
    new: "Neu",
    preparing_documents: "Unterlagen vorbereiten",
    applied: "Beworben",
    response: "Rückmeldung",
    first_interview: "Erstgespräch",
    technical_interview: "Technisches Interview",
    offer: "Angebot",
    rejected: "Abgelehnt",
    withdrawn: "Zurückgezogen",
    archived: "Archiviert",
  },
  dashboard: {
    kpis: {
      comparedToPreviousWeek: "vs. letzte Woche",
      open: "offen",
      clear: "ok",
      newPostings: {
        title: "Neue Stellenanzeigen",
      },
      interestingPostings: {
        title: "Interessante Stellenanzeigen",
        newThisWeek: "{{count}} neu diese Woche",
      },
      applicationsSent: {
        title: "Beworben",
      },
      followUpsDue: {
        title: "Follow-ups fällig",
        dueUntilToday: "bis heute",
      },
    },
  },
} as const
