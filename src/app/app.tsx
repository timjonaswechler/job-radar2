import { Suspense, useEffect, useState } from "react";

import { getAppRoute } from "@/app/navigation/app-routes";
import { APP_ROUTE_CHANGE_EVENT } from "@/app/navigation/path";
import { AppLayout } from "@/components/layout/app-layout";
import { PostingsWorkspaceProvider } from "@/features/postings/postings-workspace-provider";

export function App() {
  const [pathname, setPathname] = useState(() => window.location.pathname);

  useEffect(() => {
    const handleRouteChange = () => setPathname(window.location.pathname);

    window.addEventListener(APP_ROUTE_CHANGE_EVENT, handleRouteChange);
    window.addEventListener("popstate", handleRouteChange);

    return () => {
      window.removeEventListener(APP_ROUTE_CHANGE_EVENT, handleRouteChange);
      window.removeEventListener("popstate", handleRouteChange);
    };
  }, []);

  const route = getAppRoute(pathname);
  const Page = route.Component;

  return (
    <PostingsWorkspaceProvider pathname={pathname}>
      <AppLayout title={route.title}>
        <Suspense
          fallback={<div className="text-sm text-muted-foreground">Lädt…</div>}
        >
          <Page />
        </Suspense>
      </AppLayout>
    </PostingsWorkspaceProvider>
  );
}
