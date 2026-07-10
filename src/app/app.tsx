import { Suspense, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { getAppRoute } from "@/app/navigation/app-routes";
import { APP_ROUTE_CHANGE_EVENT } from "@/app/navigation/path";
import { AppLayout } from "@/components/layout/app-layout";
import { PostingsWorkspaceProvider } from "@/features/postings/workspace/postings-workspace-provider";
import { getAppPreferences } from "@/lib/api/app-preferences";
import {
  applyWindowControlPlatform,
  detectWindowControlPlatform,
  isWindowControlPlatformOverrideStorageKey,
  isWindowDragRegionStorageKey,
  readStoredWindowDragRegionEnabled,
  WINDOW_DRAG_REGION_PREFERENCE_CHANGED_EVENT,
} from "@/lib/window-chrome";
import { writeStoredWindowDragRegionEnabled } from "@/lib/app-settings";

export function App() {
  const { t } = useTranslation();
  const [pathname, setPathname] = useState(() => window.location.pathname);
  const previousPathname = useRef(pathname);
  const [windowDragRegionEnabled, setWindowDragRegionEnabled] = useState(() =>
    readStoredWindowDragRegionEnabled(),
  );

  useEffect(() => {
    const handleRouteChange = () => setPathname(window.location.pathname);

    window.addEventListener(APP_ROUTE_CHANGE_EVENT, handleRouteChange);
    window.addEventListener("popstate", handleRouteChange);

    return () => {
      window.removeEventListener(APP_ROUTE_CHANGE_EVENT, handleRouteChange);
      window.removeEventListener("popstate", handleRouteChange);
    };
  }, []);

  useEffect(() => {
    applyWindowControlPlatform(detectWindowControlPlatform());

    const handleStorageChange = (event: StorageEvent) => {
      if (!isWindowControlPlatformOverrideStorageKey(event.key)) return;
      applyWindowControlPlatform(detectWindowControlPlatform());
    };

    window.addEventListener("storage", handleStorageChange);
    return () => window.removeEventListener("storage", handleStorageChange);
  }, []);

  useEffect(() => {
    let cancelled = false;

    void getAppPreferences()
      .then((preferences) => {
        if (cancelled) return;
        setWindowDragRegionEnabled(preferences.windowDragRegionEnabled);
        writeStoredWindowDragRegionEnabled(preferences.windowDragRegionEnabled);
      })
      .catch((error) => {
        console.warn("Could not read window drag region preference", error);
      });

    const handlePreferenceChanged = (event: Event) => {
      const enabled = (event as CustomEvent<{ enabled: boolean }>).detail
        ?.enabled;
      if (typeof enabled === "boolean") {
        setWindowDragRegionEnabled(enabled);
      }
    };

    const handleStorageChange = (event: StorageEvent) => {
      if (!isWindowDragRegionStorageKey(event.key)) return;
      setWindowDragRegionEnabled(readStoredWindowDragRegionEnabled());
    };

    window.addEventListener(
      WINDOW_DRAG_REGION_PREFERENCE_CHANGED_EVENT,
      handlePreferenceChanged,
    );
    window.addEventListener("storage", handleStorageChange);

    return () => {
      cancelled = true;
      window.removeEventListener(
        WINDOW_DRAG_REGION_PREFERENCE_CHANGED_EVENT,
        handlePreferenceChanged,
      );
      window.removeEventListener("storage", handleStorageChange);
    };
  }, []);

  const route = getAppRoute(pathname);
  const title = t(route.titleKey);
  const Page = route.Component;

  useEffect(() => {
    document.title = `${title} · Job Radar`;
  }, [title]);

  useEffect(() => {
    if (previousPathname.current === pathname) return;
    previousPathname.current = pathname;

    const animationFrame = window.requestAnimationFrame(() => {
      document.getElementById("main-content")?.focus({ preventScroll: true });
    });

    return () => window.cancelAnimationFrame(animationFrame);
  }, [pathname]);

  return (
    <PostingsWorkspaceProvider pathname={pathname}>
      <AppLayout
        pathname={pathname}
        title={title}
        windowDragRegionEnabled={windowDragRegionEnabled}
      >
        <Suspense
          fallback={<div className="text-sm text-muted-foreground">Lädt…</div>}
        >
          <Page />
        </Suspense>
      </AppLayout>
    </PostingsWorkspaceProvider>
  );
}
