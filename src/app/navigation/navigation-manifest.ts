import { lazy } from "react";
import {
  InboxIcon,
  LayoutDashboardIcon,
  RadarIcon,
  SearchCheckIcon,
  SettingsIcon,
} from "lucide-react";

import type {
  NavigationManifestItem,
  SidebarNavigationGroup,
} from "@/app/navigation/navigation-types";
import { HomePage } from "@/pages/home-page";

const PostingsPage = lazy(() =>
  import("@/pages/postings-page").then((module) => ({
    default: module.PostingsPage,
  })),
);

const SourcesPage = lazy(() =>
  import("@/pages/sources-page").then((module) => ({
    default: module.SourcesPage,
  })),
);

const SearchRequestsPage = lazy(() =>
  import("@/pages/search-requests-page").then((module) => ({
    default: module.SearchRequestsPage,
  })),
);

const SettingsPage = lazy(() =>
  import("@/pages/settings-page").then((module) => ({
    default: module.SettingsPage,
  })),
);

const primarySidebarGroup = {
  groupId: "job-radar",
  groupLabelKey: "navigation.groups.jobRadar",
} as const;

export const navigationManifest = [
  {
    id: "overview",
    path: "/",
    titleKey: "navigation.items.overview",
    icon: LayoutDashboardIcon,
    Component: HomePage,
    sidebar: primarySidebarGroup,
    commandSearch: { groupKey: "commandSearch.groups.pages" },
  },
  {
    id: "postings",
    path: "/postings",
    titleKey: "navigation.items.postings",
    icon: InboxIcon,
    Component: PostingsPage,
    commandSearch: { groupKey: "commandSearch.groups.pages" },
  },
  {
    id: "sources",
    path: "/sources",
    titleKey: "navigation.items.sources",
    icon: RadarIcon,
    Component: SourcesPage,
    sidebar: primarySidebarGroup,
    commandSearch: { groupKey: "commandSearch.groups.pages" },
  },
  {
    id: "search-requests",
    path: "/search-requests",
    titleKey: "navigation.items.searchRequests",
    icon: SearchCheckIcon,
    Component: SearchRequestsPage,
    sidebar: primarySidebarGroup,
    commandSearch: { groupKey: "commandSearch.groups.pages" },
  },
  {
    id: "settings",
    path: "/settings",
    titleKey: "navigation.items.settings",
    icon: SettingsIcon,
    Component: SettingsPage,
    sidebar: primarySidebarGroup,
    commandSearch: { groupKey: "commandSearch.groups.pages" },
  },
] as const satisfies readonly NavigationManifestItem[];

export type NavigationManifestEntry = (typeof navigationManifest)[number];
export type NavigationId = NavigationManifestEntry["id"];

export const sidebarNavigationGroups = buildSidebarNavigationGroups(
  navigationManifest,
);

export function getNavigationItem(id: NavigationId): NavigationManifestEntry {
  const item = navigationManifest.find((candidate) => candidate.id === id);
  if (!item) {
    throw new Error(`Unknown navigation item: ${id}`);
  }
  return item;
}

function buildSidebarNavigationGroups(
  items: readonly NavigationManifestEntry[],
): ReadonlyArray<
  Omit<SidebarNavigationGroup, "items"> & {
    items: NavigationManifestEntry[];
  }
> {
  const groups: Array<{
    id: string;
    labelKey?: SidebarNavigationGroup["labelKey"];
    items: NavigationManifestEntry[];
  }> = [];

  for (const item of items) {
    if (!("sidebar" in item)) continue;

    const sidebar = item.sidebar;
    const existingGroup = groups.find(
      (group) => group.id === sidebar.groupId,
    );
    if (existingGroup) {
      existingGroup.items.push(item);
      continue;
    }

    groups.push({
      id: sidebar.groupId,
      labelKey: sidebar.groupLabelKey,
      items: [item],
    });
  }

  return groups;
}
