import { ApplicationsPage } from "@/pages/applications/page"
import { DashboardPage } from "@/pages/dashboard/page"
import { DataPage } from "@/pages/data"
import { JobSourcesPage } from "@/pages/job-sources"
import { NotFoundPage } from "@/pages/not-found"
import { PostingsInboxPage } from "@/pages/postings-inbox"
import { RemindersPage } from "@/pages/reminders"
import { SearchQueriesPage } from "@/pages/search-queries"
import { SearchRunsPage } from "@/pages/search-runs"
import { SettingsPage } from "@/pages/settings"
import { MailsPage } from "@/pages/mails/page"

export type AppRoute = {
  path: string
  title: string
  Component: () => React.ReactNode
}

export const appRoutes: AppRoute[] = [
  {
    path: "/",
    title: "Übersicht",
    Component: DashboardPage,
  },
  {
    path: "/mails",
    title: "Mails",
    Component: MailsPage,
  },
  {
    path: "/stellenanzeigen",
    title: "Stellenanzeigen-Inbox",
    Component: PostingsInboxPage,
  },
  {
    path: "/bewerbungen",
    title: "Bewerbungen",
    Component: ApplicationsPage,
  },
  {
    path: "/suchanfragen",
    title: "Suchanfragen",
    Component: SearchQueriesPage,
  },
  {
    path: "/jobquellen",
    title: "Jobquellen",
    Component: JobSourcesPage,
  },
  {
    path: "/suchlaeufe",
    title: "Suchläufe",
    Component: SearchRunsPage,
  },
  {
    path: "/erinnerungen",
    title: "Erinnerungen",
    Component: RemindersPage,
  },
  {
    path: "/daten",
    title: "Import & Export",
    Component: DataPage,
  },
  {
    path: "/einstellungen",
    title: "Einstellungen",
    Component: SettingsPage,
  },
]

export function getAppRoute(pathname: string): AppRoute {
  return (
    appRoutes.find((route) => route.path === pathname) ?? {
      path: pathname,
      title: "Nicht gefunden",
      Component: NotFoundPage,
    }
  )
}
