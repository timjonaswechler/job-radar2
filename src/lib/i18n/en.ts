export const en = {
  app: {
    title: "Job Radar",
    description:
      "Local desktop app for managing job postings, applications, search runs, and reminders.",
    openGithubRepository: "Open GitHub repository",
  },
  language: {
    selectLanguage: "Select language",
    switchToEnglish: "Switch to English",
    switchToGerman: "Switch to German",
  },
  navigation: {
    newApplication: "New Application",
    groups: {
      jobRadar: "Job Radar",
      search: "Search",
      system: "System",
      other: "Other",
    },
    items: {
      overview: "Overview",
      mails: "Mails",
      postingsInbox: "Postings Inbox",
      applications: "Applications",
      reminders: "Reminders",
      searchQueries: "Search Queries",
      jobSources: "Job Sources",
      searchRuns: "Search Runs",
      data: "Import & Export",
      history: "History",
      settings: "Settings",
    },
  },
  commandSearch: {
    button: "Search",
    placeholder: "Open pages, sections, and actions…",
    noResults: "No results found.",
    soon: "Soon",
    groups: {
      pages: "Pages",
      sections: "Sections",
      actions: "Actions",
    },
    sections: {
      activeApplications: "Active Applications",
      language: "Language",
      import: "Import",
    },
    actions: {
      newApplication: "New Application",
      startSearchRun: "Start Search Run",
      newSearchQuery: "New Search Query",
    },
  },
  search: {
    button: "Search",
    placeholder: "Open postings, applications, and search…",
    noResults: "No results found.",
    soon: "Soon",
  },
  theme: {
    switchToDark: "Switch to dark mode",
    switchToLight: "Switch to light mode",
  },
  applicationStatus: {
    new: "New",
    preparing_documents: "Preparing documents",
    applied: "Applied",
    response: "Response",
    first_interview: "First interview",
    technical_interview: "Technical interview",
    offer: "Offer",
    rejected: "Rejected",
    withdrawn: "Withdrawn",
    archived: "Archived",
  },
  dashboard: {
    kpis: {
      comparedToPreviousWeek: "vs. last week",
      open: "open",
      clear: "ok",
      newPostings: {
        title: "New postings",
      },
      interestingPostings: {
        title: "Interesting postings",
        newThisWeek: "{{count}} new this week",
      },
      applicationsSent: {
        title: "Applied",
      },
      followUpsDue: {
        title: "Follow-ups due",
        dueUntilToday: "due by today",
      },
    },
  },
} as const
