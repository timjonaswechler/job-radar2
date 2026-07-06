import {
  LayoutDashboard,
  Radar,
  SearchCheckIcon,
  Settings,
  type LucideIcon,
} from "lucide-react";
import type { TranslationKey } from "@/lib/i18n/resources";

export interface NavSubItem {
  titleKey: TranslationKey;
  url: string;
  icon?: LucideIcon;
  comingSoon?: boolean;
  newTab?: boolean;
  isNew?: boolean;
}

export interface NavMainItem {
  titleKey: TranslationKey;
  url: string;
  icon?: LucideIcon;
  subItems?: NavSubItem[];
  comingSoon?: boolean;
  newTab?: boolean;
  isNew?: boolean;
}

export interface NavGroup {
  id: number;
  labelKey?: TranslationKey;
  items: NavMainItem[];
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
        titleKey: "navigation.items.sources",
        url: "/sources",
        icon: Radar,
      },
      {
        titleKey: "navigation.items.searchRequests",
        url: "/search-requests",
        icon: SearchCheckIcon,
      },
      {
        titleKey: "navigation.items.settings",
        url: "/settings",
        icon: Settings,
      },
    ],
  },
];
