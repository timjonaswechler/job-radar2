import { lazy, type ComponentType } from "react";

import { HomePage } from "@/pages/home-page";

const SourcesPage = lazy(() =>
  import("@/pages/sources-page").then((module) => ({
    default: module.SourcesPage,
  })),
);

const SettingsPage = lazy(() =>
  import("@/pages/settings-page").then((module) => ({
    default: module.SettingsPage,
  })),
);

export type AppRoute = {
  path: string;
  title: string;
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

export const appRoutes: AppRoute[] = [
  {
    path: "/",
    title: "Übersicht",
    Component: HomePage,
  },
  {
    path: "/sources",
    title: "Quellen",
    Component: SourcesPage,
  },
  {
    path: "/settings",
    title: "Einstellungen",
    Component: SettingsPage,
  },
];

export function getAppRoute(pathname: string): AppRoute {
  return (
    appRoutes.find((route) => route.path === pathname) ?? {
      path: pathname,
      title: "Nicht gefunden",
      Component: NotFoundPage,
    }
  );
}
