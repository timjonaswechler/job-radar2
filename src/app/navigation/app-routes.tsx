import type { ComponentType } from "react";

import { navigationManifest } from "@/app/navigation/navigation-manifest";
import type { NavigationManifestItem } from "@/app/navigation/navigation-types";
import { isAppPathActive } from "@/app/navigation/path";
import type { TranslationKey } from "@/lib/i18n/resources";

export type AppRoute = Pick<
  NavigationManifestItem,
  "id" | "path" | "titleKey" | "Component"
>;

type NotFoundRoute = {
  id: "not-found";
  path: string;
  titleKey: TranslationKey;
  Component: ComponentType;
};

function NotFoundPage() {
  return (
    <div className="rounded-lg border bg-card p-6 text-card-foreground">
      <h1 className="text-lg font-semibold">Seite nicht gefunden</h1>
      <p className="mt-2 text-sm text-muted-foreground">
        Für diesen Pfad ist noch keine Seite registriert.
      </p>
    </div>
  );
}

export const appRoutes: readonly AppRoute[] = navigationManifest.map(
  ({ id, path, titleKey, Component }) => ({
    id,
    path,
    titleKey,
    Component,
  }),
);

export function getAppRoute(pathname: string): AppRoute | NotFoundRoute {
  let route: AppRoute | undefined;

  for (const candidate of appRoutes) {
    if (
      isAppPathActive(pathname, candidate.path) &&
      (!route || candidate.path.length > route.path.length)
    ) {
      route = candidate;
    }
  }

  return (
    route ?? {
      id: "not-found",
      path: pathname,
      titleKey: "navigation.items.notFound",
      Component: NotFoundPage,
    }
  );
}
