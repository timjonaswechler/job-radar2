import { useEffect, useMemo, useRef, useState } from "react";

import { AlertCircleIcon } from "lucide-react";
import { toast } from "sonner";

import { APP_ROUTE_CHANGE_EVENT } from "@/app/navigation/path";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AddRegistryDocumentDrawer } from "@/features/sources/add/add-registry-document-drawer";
import { ProfileRegistryTab } from "@/features/sources/registry/profile/profile-registry-tab";
import { SourceRegistryTab } from "@/features/sources/registry/source/source-registry-tab";
import { BrowserRuntimeCard } from "@/features/sources/runtime/browser-runtime-card";
import {
  buildDiagnosticIndex,
  diagnosticCountLabel,
} from "@/features/sources/view-model/diagnostics";
import { CustomRegistryFoldersCard } from "@/features/sources/workspace/custom-registry-folders-card";
import { SourcesDiagnosticsTab } from "@/features/sources/workspace/sources-diagnostics-tab";
import {
  parseSourcesWorkspaceTab,
  type SourcesWorkspaceTab,
  updateSourcesWorkspaceTab,
} from "@/features/sources/workspace/sources-workspace-tabs";
import { useSourceRegistryInventory } from "@/features/sources/workspace/use-source-registry-inventory";
import { useDatabaseInfo } from "@/hooks/use-database-info";
import type { SourceRegistryDocumentKind } from "@/lib/api/sources";

export function SourcesWorkspaceView() {
  const { data, error, loading, refresh } = useSourceRegistryInventory();
  const databaseInfo = useDatabaseInfo();
  const [activeTab, setActiveTab] = useState<SourcesWorkspaceTab>(() =>
    parseSourcesWorkspaceTab(window.location.search),
  );
  const [addDrawerKind, setAddDrawerKind] =
    useState<SourceRegistryDocumentKind | null>(null);
  const initialDiagnosticToastShown = useRef(false);

  const profiles = data?.profiles ?? [];
  const sources = data?.sources ?? [];
  const diagnostics = data?.diagnostics ?? [];

  const profilesByKey = useMemo(
    () => new Map(profiles.map((profile) => [profile.document.key, profile])),
    [profiles],
  );
  const diagnosticIndex = useMemo(
    () => buildDiagnosticIndex(sources, profiles, diagnostics),
    [diagnostics, profiles, sources],
  );

  useEffect(() => {
    const handleLocationChange = () => {
      setActiveTab(parseSourcesWorkspaceTab(window.location.search));
    };

    window.addEventListener(APP_ROUTE_CHANGE_EVENT, handleLocationChange);
    window.addEventListener("popstate", handleLocationChange);

    return () => {
      window.removeEventListener(APP_ROUTE_CHANGE_EVENT, handleLocationChange);
      window.removeEventListener("popstate", handleLocationChange);
    };
  }, []);

  useEffect(() => {
    if (
      initialDiagnosticToastShown.current ||
      loading ||
      error ||
      !data ||
      !diagnostics.length
    ) {
      return;
    }

    initialDiagnosticToastShown.current = true;
    toast.warning(
      `${diagnosticCountLabel(diagnostics.length)} in der Source Registry`,
      {
        description:
          "Marker an betroffenen Quellen/Profilen und die Diagnosen-Liste prüfen.",
      },
    );
  }, [data, diagnostics.length, error, loading]);

  const handleTabChange = (value: string) => {
    const tab = parseSourcesWorkspaceTab(
      new URLSearchParams({ tab: value }).toString(),
    );
    setActiveTab(tab);
    updateSourcesWorkspaceTab(tab);
  };

  return (
    <div className="grid gap-4">
      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Source Registry konnte nicht geladen werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <CustomRegistryFoldersCard
        data={databaseInfo.data}
        error={databaseInfo.error}
        loading={databaseInfo.loading}
        onRefresh={databaseInfo.refresh}
      />

      <Tabs
        value={activeTab}
        onValueChange={handleTabChange}
        className="grid gap-4"
      >
        <TabsList className="flex-wrap justify-start">
          <TabsTrigger value="sources">Quellen ({sources.length})</TabsTrigger>
          <TabsTrigger value="profiles">
            Profile ({profiles.length})
          </TabsTrigger>
          <TabsTrigger value="diagnostics">
            Diagnosen ({diagnostics.length})
          </TabsTrigger>
          <TabsTrigger value="runtime">Browser-Laufzeit</TabsTrigger>
        </TabsList>

        <TabsContent value="sources">
          <SourceRegistryTab
            sources={sources}
            profilesByKey={profilesByKey}
            diagnosticIndex={diagnosticIndex}
            loading={loading}
            onAdd={() => setAddDrawerKind("source")}
            onUpdated={refresh}
          />
        </TabsContent>

        <TabsContent value="profiles">
          <ProfileRegistryTab
            profiles={profiles}
            diagnosticIndex={diagnosticIndex}
            loading={loading}
            onAdd={() => setAddDrawerKind("source_profile")}
          />
        </TabsContent>

        <TabsContent value="diagnostics" className="grid gap-3">
          <SourcesDiagnosticsTab
            diagnostics={diagnostics}
            unassignedDiagnostics={diagnosticIndex.unassigned}
            loading={loading}
          />
        </TabsContent>

        <TabsContent value="runtime">
          <BrowserRuntimeCard />
        </TabsContent>
      </Tabs>

      <AddRegistryDocumentDrawer
        kind={addDrawerKind}
        open={addDrawerKind !== null}
        profiles={profiles}
        sources={sources}
        onCreated={refresh}
        onOpenChange={(open) => {
          if (!open) setAddDrawerKind(null);
        }}
      />
    </div>
  );
}
