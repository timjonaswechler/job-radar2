import { navigateTo } from "@/app/navigation/path";

export const sourcesWorkspaceTabs = [
  "sources",
  "profiles",
  "diagnostics",
  "runtime",
] as const;

export type SourcesWorkspaceTab = (typeof sourcesWorkspaceTabs)[number];

type SourcesWorkspaceLocation = Pick<
  Location,
  "pathname" | "search" | "hash"
>;

export function parseSourcesWorkspaceTab(search: string): SourcesWorkspaceTab {
  const value = new URLSearchParams(search).get("tab");
  return isSourcesWorkspaceTab(value) ? value : "sources";
}

export function sourcesWorkspaceTabUrl(
  tab: SourcesWorkspaceTab,
  location: SourcesWorkspaceLocation,
) {
  const searchParams = new URLSearchParams(location.search);
  searchParams.set("tab", tab);
  const search = searchParams.toString();

  return `${location.pathname}${search ? `?${search}` : ""}${location.hash}`;
}

export function updateSourcesWorkspaceTab(
  tab: SourcesWorkspaceTab,
  location: SourcesWorkspaceLocation = window.location,
) {
  navigateTo(sourcesWorkspaceTabUrl(tab, location));
}

function isSourcesWorkspaceTab(
  value: string | null,
): value is SourcesWorkspaceTab {
  return sourcesWorkspaceTabs.some((tab) => tab === value);
}
