import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { AlertCircleIcon, CheckCircle2Icon } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { BrowserRuntimeCard } from "@/features/sources/runtime/browser-runtime-card";
import { DiagnosticCard } from "@/features/sources/registry/registry-diagnostics";
import { ProfileRegistryTab } from "@/features/sources/registry/profile/profile-registry-tab";
import { AddRegistryDocumentDrawer } from "@/features/sources/add/add-registry-document-drawer";
import { SourceRegistryTab } from "@/features/sources/registry/source/source-registry-tab";
import {
  buildDiagnosticIndex,
  diagnosticCountLabel,
  type SourceRegistryInventory,
} from "@/features/sources/view-model/registry-view-model";
import {
  getSourceProfileRegistrySnapshot,
  type SourceRegistryDocumentKind,
} from "@/lib/api/sources";

function useSourceRegistryInventory() {
  const [data, setData] = useState<SourceRegistryInventory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const nextData = await getSourceProfileRegistrySnapshot();
      setData(nextData);
      return nextData;
    } catch (unknownError) {
      const message = errorMessage(unknownError);
      setData(null);
      setError(message);
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { data, error, loading, refresh };
}

export function SourcesWorkspaceView() {
  const { data, error, loading, refresh } = useSourceRegistryInventory();
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

  return (
    <div className="grid gap-4">
      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Source Registry konnte nicht geladen werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <Tabs defaultValue="sources" className="grid gap-4">
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
          {loading ? (
            <DiagnosticsSkeleton />
          ) : diagnostics.length ? (
            <>
              {diagnosticIndex.unassigned.length ? (
                <Alert variant="warning">
                  <AlertCircleIcon className="size-4" aria-hidden="true" />
                  <AlertTitle>
                    {diagnosticCountLabel(diagnosticIndex.unassigned.length)}{" "}
                    ohne gültige Source-/Profil-Zeile
                  </AlertTitle>
                  <AlertDescription>
                    Diese Diagnosen gehören zu Registry-Dokumenten, die nicht
                    als gültige Quelle oder gültiges Profil geladen wurden. Sie
                    bleiben hier global sichtbar.
                  </AlertDescription>
                </Alert>
              ) : null}
              <div className="grid gap-3 md:grid-cols-2">
                {diagnostics.map((diagnostic, index) => (
                  <div
                    key={`${diagnostic.path}-${diagnostic.code}-${index}`}
                    className="[contain-intrinsic-size:220px] [content-visibility:auto]"
                  >
                    <DiagnosticCard diagnostic={diagnostic} />
                  </div>
                ))}
              </div>
            </>
          ) : (
            <Alert variant="success">
              <CheckCircle2Icon className="size-4" aria-hidden="true" />
              <AlertTitle>Keine Registry-Diagnosen</AlertTitle>
              <AlertDescription>
                Alle geladenen Source-Registry-Dokumente sind gültig und ihre
                Profil-/Zugriffspfad-Referenzen konnten aufgelöst werden.
              </AlertDescription>
            </Alert>
          )}
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

function DiagnosticsSkeleton() {
  return (
    <div className="grid gap-3 md:grid-cols-2">
      {Array.from({ length: 4 }).map((_, index) => (
        <Card key={index}>
          <CardHeader>
            <Skeleton className="h-5 w-1/2" />
            <Skeleton className="h-4 w-2/3" />
          </CardHeader>
          <CardContent className="grid gap-2">
            <Skeleton className="h-4 w-1/3" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-5/6" />
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
