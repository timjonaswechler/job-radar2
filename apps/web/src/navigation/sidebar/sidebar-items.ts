import {
  Bell,
  BriefcaseBusiness,
  Building2,
  Database,
  History,
  Inbox,
  LayoutDashboard,
  Radar,
  Search,
  Settings,
  type LucideIcon,
} from "lucide-react"

export interface NavSubItem {
  titleKey: string
  url: string
  icon?: LucideIcon
  comingSoon?: boolean
  newTab?: boolean
  isNew?: boolean
}

export interface NavMainItem {
  titleKey: string
  url: string
  icon?: LucideIcon
  subItems?: NavSubItem[]
  comingSoon?: boolean
  newTab?: boolean
  isNew?: boolean
}

export interface NavGroup {
  id: number
  labelKey?: string
  items: NavMainItem[]
}

export const sidebarItems: NavGroup[] = [
  {
    id: 1,
    labelKey: "navigation.groups.jobRadar",
    items: [
      {
        titleKey: "navigation.items.overview",
        url: "/",
        icon: LayoutDashboard,
      },
      {
        titleKey: "navigation.items.postingsInbox",
        url: "/stellenanzeigen",
        icon: Inbox,
      },
      {
        titleKey: "navigation.items.applications",
        url: "/bewerbungen",
        icon: BriefcaseBusiness,
      },
      {
        titleKey: "navigation.items.reminders",
        url: "/erinnerungen",
        icon: Bell,
      },
    ],
  },
  {
    id: 2,
    labelKey: "navigation.groups.search",
    items: [
      {
        titleKey: "navigation.items.searchQueries",
        url: "/suchanfragen",
        icon: Search,
      },
      {
        titleKey: "navigation.items.jobSources",
        url: "/jobquellen",
        icon: Building2,
      },
      {
        titleKey: "navigation.items.searchRuns",
        url: "/suchlaeufe",
        icon: Radar,
      },
    ],
  },
  {
    id: 3,
    labelKey: "navigation.groups.system",
    items: [
      {
        titleKey: "navigation.items.data",
        url: "/daten",
        icon: Database,
      },
      {
        titleKey: "navigation.items.history",
        url: "/historie",
        icon: History,
        comingSoon: true,
      },
      {
        titleKey: "navigation.items.settings",
        url: "/einstellungen",
        icon: Settings,
      },
    ],
  },
]
