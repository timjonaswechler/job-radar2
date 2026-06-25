import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  AlertCircleIcon,
  CheckCircle2Icon,
  FileJsonIcon,
  XIcon,
} from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { BrowserRuntimeCard } from "@/features/sources/components/browser-runtime-card";
import { DiagnosticCard } from "@/features/sources/components/registry-diagnostics";
import { ProfileRegistryDataGrid } from "@/features/sources/components/profile-registry-data-grid";
import { SourceAddDrawer } from "@/features/sources/components/source-add-drawer";
import { SourceRegistryDataGrid } from "@/features/sources/components/source-registry-data-grid";
import { documentDirectoryLabels } from "@/features/sources/labels";
import {
  buildDiagnosticIndex,
  diagnosticCountLabel,
  type SourceRegistryInventory,
} from "@/features/sources/registry-view-model";
import {
  listAdapters,
  listSourceRegistryDiagnostics,
  listSourceRegistryProfiles,
  listSourceRegistrySources,
  type JsonValue,
  type RegistrySource,
  type RegistrySourceProfile,
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
      const [adapters, profiles, sources, diagnostics] = await Promise.all([
        listAdapters(),
        listSourceRegistryProfiles(),
        listSourceRegistrySources(),
        listSourceRegistryDiagnostics(),
      ]);
      const nextData = { adapters, profiles, sources, diagnostics };
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

export function SourcesFeature() {
  const { data, error, loading, refresh } = useSourceRegistryInventory();
  const [addDrawerKind, setAddDrawerKind] =
    useState<SourceRegistryDocumentKind | null>(null);
  const initialDiagnosticToastShown = useRef(false);

  const adapters = data?.adapters ?? [];
  const profiles = data?.profiles ?? [];
  const sources = data?.sources ?? [];
  const diagnostics = data?.diagnostics ?? [];

  const adaptersByKey = useMemo(
    () => new Map(adapters.map((adapter) => [adapter.key, adapter])),
    [adapters],
  );
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
    <div className="grid gap-4 p-2">
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
          <SourceRegistryDataGrid
            sources={sources}
            profilesByKey={profilesByKey}
            adaptersByKey={adaptersByKey}
            diagnosticIndex={diagnosticIndex}
            loading={loading}
            onAdd={() => setAddDrawerKind("source")}
          />
        </TabsContent>

        <TabsContent value="profiles">
          <ProfileRegistryDataGrid
            profiles={profiles}
            adaptersByKey={adaptersByKey}
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
                  <DiagnosticCard
                    key={`${diagnostic.path}-${diagnostic.code}-${index}`}
                    diagnostic={diagnostic}
                  />
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

type AddRegistryDocumentDrawerProps = {
  kind: SourceRegistryDocumentKind | null;
  open: boolean;
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  onCreated?: () => Promise<unknown> | unknown;
  onOpenChange: (open: boolean) => void;
};

function AddRegistryDocumentDrawer({
  kind,
  open,
  profiles,
  sources,
  onCreated,
  onOpenChange,
}: AddRegistryDocumentDrawerProps) {
  if (!kind) {
    return <Drawer open={open} onOpenChange={onOpenChange} direction="right" />;
  }

  if (kind === "source") {
    return (
      <SourceAddDrawer
        open={open}
        profiles={profiles}
        sources={sources}
        onCreated={onCreated}
        onOpenChange={onOpenChange}
      />
    );
  }

  const title = "Quellenprofil hinzufügen";
  const directory = documentDirectoryLabels[kind];
  const snippet = profileTemplateSnippet;

  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction="right">
      <DrawerContent className="h-full sm:max-w-xl lg:max-w-2xl">
        <DrawerHeader className="border-b pr-12">
          <DrawerTitle>{title}</DrawerTitle>
          <DrawerDescription>
            Add legt keinen DB-Datensatz an. Erstelle stattdessen ein
            Registry-JSON-Dokument mit passendem Dateinamen im App-Data-Ordner.
          </DrawerDescription>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="absolute top-5 right-5"
            onClick={() => onOpenChange(false)}
          >
            <XIcon aria-hidden="true" />
            <span className="sr-only">Drawer schließen</span>
          </Button>
        </DrawerHeader>
        <div className="grid min-h-0 gap-4 overflow-y-auto p-4 text-sm">
          <Alert>
            <FileJsonIcon aria-hidden="true" />
            <AlertTitle>JSON-Registry-Dokument anlegen</AlertTitle>
            <AlertDescription>
              Datei als <code>{directory}</code> speichern. Der Dateiname muss
              exakt dem <code>key</code> im JSON entsprechen, z. B.
              <code className="mx-1">example_profile.json</code>.
            </AlertDescription>
          </Alert>
          <div className="grid gap-2">
            <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              Minimaler Startpunkt
            </h3>
            <pre className="max-h-96 overflow-auto rounded-md bg-muted p-3 font-mono text-xs">
              {JSON.stringify(snippet, null, 2)}
            </pre>
          </div>
        </div>
      </DrawerContent>
    </Drawer>
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

const profileTemplateSnippet: JsonValue = {
  schemaVersion: 1,
  key: "example_profile",
  name: "Example Profile",
  kind: "generic",
  accessPaths: [
    {
      key: "endpoint_inventory",
      adapterKey: "declarative_endpoint_inventory",
      sourceConfigSchema: {
        type: "object",
        properties: {
          startUrl: {
            type: "string",
            format: "uri",
          },
        },
      },
      inventory: {},
    },
  ],
};
