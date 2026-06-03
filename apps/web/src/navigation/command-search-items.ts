import {
  BriefcaseBusiness,
  Languages,
  PlayCircleIcon,
  PlusCircleIcon,
  SearchIcon,
  UploadIcon,
  type LucideIcon,
} from "lucide-react"

import type { TranslationKey } from "@/lib/i18n/resources"
import { sidebarItems } from "@/navigation/sidebar/sidebar-items"

export type CommandSearchItemKind = "page" | "section" | "action"

export type CommandSearchItem = {
  id: string
  kind: CommandSearchItemKind
  groupKey: TranslationKey
  titleKey: TranslationKey
  parentTitleKey?: TranslationKey
  url: string
  icon?: LucideIcon
  disabled?: boolean
  newTab?: boolean
}

const pageCommandSearchItems: CommandSearchItem[] = sidebarItems.flatMap(
  (group) =>
    group.items.flatMap((item) => {
      if (item.subItems?.length) {
        return item.subItems.map((subItem) => ({
          id: `page:${subItem.url}`,
          kind: "page",
          groupKey: "commandSearch.groups.pages",
          titleKey: subItem.titleKey,
          parentTitleKey: item.titleKey,
          url: subItem.url,
          icon: subItem.icon ?? item.icon,
          disabled: subItem.comingSoon,
          newTab: subItem.newTab,
        })) satisfies CommandSearchItem[]
      }

      return [
        {
          id: `page:${item.url}`,
          kind: "page",
          groupKey: "commandSearch.groups.pages",
          titleKey: item.titleKey,
          url: item.url,
          icon: item.icon,
          disabled: item.comingSoon,
          newTab: item.newTab,
        },
      ] satisfies CommandSearchItem[]
    })
)

const sectionCommandSearchItems: CommandSearchItem[] = [
  {
    id: "section:applications:active",
    kind: "section",
    groupKey: "commandSearch.groups.sections",
    parentTitleKey: "navigation.items.applications",
    titleKey: "commandSearch.sections.activeApplications",
    url: "/bewerbungen#aktive-bewerbungen",
    icon: BriefcaseBusiness,
  },
  {
    id: "section:settings:language",
    kind: "section",
    groupKey: "commandSearch.groups.sections",
    parentTitleKey: "navigation.items.settings",
    titleKey: "commandSearch.sections.language",
    url: "/einstellungen#sprache",
    icon: Languages,
  },
  {
    id: "section:data:import",
    kind: "section",
    groupKey: "commandSearch.groups.sections",
    parentTitleKey: "navigation.items.data",
    titleKey: "commandSearch.sections.import",
    url: "/daten#import",
    icon: UploadIcon,
  },
]

const actionCommandSearchItems: CommandSearchItem[] = [
  {
    id: "action:application:new",
    kind: "action",
    groupKey: "commandSearch.groups.actions",
    titleKey: "commandSearch.actions.newApplication",
    url: "/bewerbungen?action=new",
    icon: PlusCircleIcon,
  },
  {
    id: "action:search-run:start",
    kind: "action",
    groupKey: "commandSearch.groups.actions",
    titleKey: "commandSearch.actions.startSearchRun",
    url: "/suchlaeufe?action=start",
    icon: PlayCircleIcon,
  },
  {
    id: "action:search-query:new",
    kind: "action",
    groupKey: "commandSearch.groups.actions",
    titleKey: "commandSearch.actions.newSearchQuery",
    url: "/suchanfragen?action=new",
    icon: SearchIcon,
  },
]

export const commandSearchItems: CommandSearchItem[] = [
  ...pageCommandSearchItems,
  ...sectionCommandSearchItems,
  ...actionCommandSearchItems,
]
