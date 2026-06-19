import { useCallback, useEffect, useMemo, useState } from "react";

import {
  AlertCircleIcon,
  CheckCircle2Icon,
  FileJsonIcon,
  RefreshCwIcon,
} from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import {
  Frame,
  FrameDescription,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getAdapterDisplay } from "@/features/sources/adapter-metadata";
import { BrowserRuntimeCard } from "@/features/sources/components/browser-runtime-card";
import {
  sourceStatusBadgeVariants,
  sourceStatusLabels,
} from "@/features/sources/status";
import {
  listAdapters,
  listSourceRegistryDiagnostics,
  listSourceRegistryProfiles,
  listSourceRegistrySources,
  type AdapterMetadata,
  type ProfileAccessPathDefinition,
  type RegistrySource,
  type RegistrySourceProfile,
  type SelectedAccessPath,
  type SourceProfileKind,
  type SourceRegistryDiagnostic,
  type SourceRegistryDiagnosticCode,
  type SourceRegistryDocumentKind,
  type SourceRegistryDocumentOrigin,
} from "@/lib/api/sources";

const originLabels: Record<SourceRegistryDocumentOrigin, string> = {
  built_in: "Eingebaut",
  custom: "Custom",
};

const documentKindLabels: Record<SourceRegistryDocumentKind, string> = {
  source_profile: "Quellenprofil",
  source: "Quelle",
};

const profileKindLabels: Record<SourceProfileKind, string> = {
  recruiting_system: "Recruiting-System",
  job_portal: "Job-Portal",
  website_family: "Website-Familie",
  generic: "Generisch",
};

const diagnosticCodeLabels: Record<SourceRegistryDiagnosticCode, string> = {
  invalid_json: "Ungültiges JSON",
  invalid_shape: "Ungültige Dokumentform",
  filename_key_mismatch: "Dateiname passt nicht zum Key",
  duplicate_key: "Doppelter Key",
  missing_profile_ref: "Fehlendes Profil",
  missing_path_ref: "Fehlender Zugriffspfad",
  read_error: "Lesefehler",
};

type SourceRegistryInventory = {
  adapters: AdapterMetadata[];
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  diagnostics: SourceRegistryDiagnostic[];
};

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
      setData({ adapters, profiles, sources, diagnostics });
    } catch (unknownError) {
      setData(null);
      setError(String(unknownError));
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
  const [search, setSearch] = useState("");
  const [selectedSourceKey, setSelectedSourceKey] = useState<string | null>(
    null,
  );

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

  const filteredSources = useMemo(
    () =>
      sources.filter((source) =>
        sourceMatchesSearch(source, search, profilesByKey, adaptersByKey),
      ),
    [adaptersByKey, profilesByKey, search, sources],
  );

  useEffect(() => {
    if (!filteredSources.length) {
      setSelectedSourceKey(null);
      return;
    }

    if (
      !selectedSourceKey ||
      !filteredSources.some(
        (source) => source.document.key === selectedSourceKey,
      )
    ) {
      setSelectedSourceKey(filteredSources[0].document.key);
    }
  }, [filteredSources, selectedSourceKey]);

  const selectedSource =
    filteredSources.find(
      (source) => source.document.key === selectedSourceKey,
    ) ?? null;

  return (
    <div className="grid gap-6">
      <Frame>
        <FramePanel>
          <FrameHeader>
            <FrameTitle>Quellen-Registry</FrameTitle>
            <FrameDescription>
              Quellen und Quellenprofile werden direkt aus JSON-Dokumenten der
              Source Registry gelesen. Suchanfragen referenzieren Quellen über
              stabile Source Keys.
            </FrameDescription>
          </FrameHeader>
          <div className="flex flex-wrap items-center gap-2 px-4 pb-4 lg:px-6">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void refresh()}
              disabled={loading}
            >
              <RefreshCwIcon
                className={loading ? "size-4 animate-spin" : "size-4"}
                aria-hidden="true"
              />
              Registry neu laden
            </Button>
            <Badge variant={diagnostics.length ? "destructive-light" : "success-light"}>
              {diagnostics.length
                ? `${diagnostics.length} Diagnose${diagnostics.length === 1 ? "" : "n"}`
                : "Keine Diagnosen"}
            </Badge>
          </div>
        </FramePanel>
      </Frame>

      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Source Registry konnte nicht geladen werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <Alert>
        <FileJsonIcon className="size-4" aria-hidden="true" />
        <AlertTitle>JSON-Authoring in diesem Schritt zurückgestellt</AlertTitle>
        <AlertDescription>
          Die frühere DB-Bearbeitung von Quellen und Profilen ist hier nicht
          mehr verfügbar. Neue oder geänderte Quellen/Profile müssen als
          Registry-JSON-Dokumente unter
          <code className="mx-1 rounded bg-muted px-1">sources/*.json</code>
          bzw.
          <code className="mx-1 rounded bg-muted px-1">
            source-profiles/*.json
          </code>
          abgelegt werden; eine reichere Authoring-UI folgt separat.
        </AlertDescription>
      </Alert>

      <Tabs defaultValue="sources" className="grid gap-4">
        <TabsList>
          <TabsTrigger value="sources">Quellen ({sources.length})</TabsTrigger>
          <TabsTrigger value="profiles">Profile ({profiles.length})</TabsTrigger>
          <TabsTrigger value="diagnostics">
            Diagnosen ({diagnostics.length})
          </TabsTrigger>
          <TabsTrigger value="runtime">Browser-Laufzeit</TabsTrigger>
        </TabsList>

        <TabsContent value="sources" className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(320px,420px)]">
          <Card>
            <CardHeader>
              <CardTitle>Gültige Quellen</CardTitle>
              <CardDescription>
                Quelle auswählen, deren Source Key in Suchanfragen verwendet
                wird.
              </CardDescription>
            </CardHeader>
            <CardContent className="grid gap-3">
              <Input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Quellen nach Name, Key, Profil oder Adapter filtern…"
              />
              {loading ? (
                <p className="text-sm text-muted-foreground">Lade Registry…</p>
              ) : filteredSources.length ? (
                <div className="grid gap-2">
                  {filteredSources.map((source) => (
                    <SourceListItem
                      key={source.document.key}
                      source={source}
                      profilesByKey={profilesByKey}
                      adaptersByKey={adaptersByKey}
                      selected={source.document.key === selectedSourceKey}
                      onSelect={() => setSelectedSourceKey(source.document.key)}
                    />
                  ))}
                </div>
              ) : (
                <p className="text-sm text-muted-foreground">
                  Keine gültigen Quellen gefunden.
                </p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Quellendetails</CardTitle>
              <CardDescription>
                Technisches Registry-Modell ohne Legacy-Datenbank-IDs.
              </CardDescription>
            </CardHeader>
            <CardContent>
              {selectedSource ? (
                <SourceDetails
                  source={selectedSource}
                  profilesByKey={profilesByKey}
                  adaptersByKey={adaptersByKey}
                />
              ) : (
                <p className="text-sm text-muted-foreground">
                  Wähle eine Quelle aus der Registry-Liste.
                </p>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="profiles" className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {loading ? (
            <p className="text-sm text-muted-foreground">Lade Profile…</p>
          ) : profiles.length ? (
            profiles.map((profile) => (
              <ProfileCard
                key={profile.document.key}
                profile={profile}
                adaptersByKey={adaptersByKey}
              />
            ))
          ) : (
            <Card>
              <CardContent className="pt-6 text-sm text-muted-foreground">
                Keine gültigen Quellenprofile gefunden.
              </CardContent>
            </Card>
          )}
        </TabsContent>

        <TabsContent value="diagnostics" className="grid gap-3">
          {diagnostics.length ? (
            diagnostics.map((diagnostic, index) => (
              <DiagnosticCard key={`${diagnostic.path}-${index}`} diagnostic={diagnostic} />
            ))
          ) : (
            <Alert>
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
    </div>
  );
}

type SourceListItemProps = {
  source: RegistrySource;
  profilesByKey: Map<string, RegistrySourceProfile>;
  adaptersByKey: Map<string, AdapterMetadata>;
  selected: boolean;
  onSelect: () => void;
};

function SourceListItem({
  source,
  profilesByKey,
  adaptersByKey,
  selected,
  onSelect,
}: SourceListItemProps) {
  const accessPath = accessPathSummary(source.document.selectedAccessPath);
  const adapterKey = sourceAdapterKey(source, profilesByKey);
  const adapter = adapterKey ? adaptersByKey.get(adapterKey) : null;
  const adapterDisplay = adapterKey
    ? getAdapterDisplay(adapterKey, adapter)
    : null;

  return (
    <button
      type="button"
      className={
        selected
          ? "rounded-lg border border-primary bg-primary/5 p-3 text-left shadow-sm"
          : "rounded-lg border bg-card p-3 text-left transition hover:border-primary/50"
      }
      onClick={onSelect}
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <p className="font-medium">{source.document.name}</p>
          <p className="font-mono text-xs text-muted-foreground">
            {source.document.key}
          </p>
        </div>
        <div className="flex flex-wrap gap-1">
          <Badge variant={sourceStatusBadgeVariants[source.document.status]}>
            {sourceStatusLabels[source.document.status]}
          </Badge>
          <Badge variant="secondary">{originLabels[source.origin]}</Badge>
        </div>
      </div>
      <div className="mt-3 grid gap-1 text-xs text-muted-foreground">
        <p>{accessPath}</p>
        {adapterDisplay ? <p>{adapterDisplay.label}</p> : null}
        <p className="break-all">{source.path}</p>
      </div>
    </button>
  );
}

type SourceDetailsProps = {
  source: RegistrySource;
  profilesByKey: Map<string, RegistrySourceProfile>;
  adaptersByKey: Map<string, AdapterMetadata>;
};

function SourceDetails({
  source,
  profilesByKey,
  adaptersByKey,
}: SourceDetailsProps) {
  const selectedAccessPath = source.document.selectedAccessPath;
  const adapterKey = sourceAdapterKey(source, profilesByKey);
  const adapter = adapterKey ? adaptersByKey.get(adapterKey) : null;
  const adapterDisplay = adapterKey
    ? getAdapterDisplay(adapterKey, adapter)
    : null;

  return (
    <div className="grid gap-4 text-sm">
      <dl className="grid gap-3">
        <DetailRow label="Source Key" value={source.document.key} mono />
        <DetailRow label="Name" value={source.document.name} />
        <DetailRow label="Status" value={sourceStatusLabels[source.document.status]} />
        <DetailRow label="Ursprung" value={originLabels[source.origin]} />
        <DetailRow label="Datei" value={source.path} mono />
        {adapterDisplay ? (
          <DetailRow label="Adapter" value={adapterDisplay.label} mono />
        ) : null}
      </dl>

      <AccessPathDetails selectedAccessPath={selectedAccessPath} />

      <div className="grid gap-1.5">
        <h3 className="text-xs font-medium uppercase text-muted-foreground">
          sourceConfig
        </h3>
        <pre className="max-h-80 overflow-auto rounded-lg bg-muted p-3 text-xs">
          {JSON.stringify(source.document.sourceConfig, null, 2)}
        </pre>
      </div>
    </div>
  );
}

type DetailRowProps = {
  label: string;
  value: string;
  mono?: boolean;
};

function DetailRow({ label, value, mono = false }: DetailRowProps) {
  return (
    <div>
      <dt className="text-xs font-medium uppercase text-muted-foreground">
        {label}
      </dt>
      <dd className={mono ? "break-all font-mono text-xs" : "break-words"}>
        {value}
      </dd>
    </div>
  );
}

type AccessPathDetailsProps = {
  selectedAccessPath: SelectedAccessPath;
};

function AccessPathDetails({ selectedAccessPath }: AccessPathDetailsProps) {
  if (selectedAccessPath.type === "profile") {
    return (
      <div className="rounded-lg border p-3 text-sm">
        <p className="font-medium">Profil-Zugriffspfad</p>
        <dl className="mt-2 grid gap-2">
          <DetailRow label="Profil-Key" value={selectedAccessPath.profileKey} mono />
          <DetailRow label="Pfad-Key" value={selectedAccessPath.pathKey} mono />
        </dl>
      </div>
    );
  }

  return (
    <div className="rounded-lg border p-3 text-sm">
      <p className="font-medium">Quellenspezifischer Zugriffspfad</p>
      <dl className="mt-2 grid gap-2">
        <DetailRow label="Adapter-Key" value={selectedAccessPath.adapterKey} mono />
      </dl>
    </div>
  );
}

type ProfileCardProps = {
  profile: RegistrySourceProfile;
  adaptersByKey: Map<string, AdapterMetadata>;
};

function ProfileCard({ profile, adaptersByKey }: ProfileCardProps) {
  const accessPaths = [...profile.document.accessPaths].sort((left, right) =>
    left.key.localeCompare(right.key, "de"),
  );

  return (
    <Card>
      <CardHeader>
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <CardTitle>{profile.document.name}</CardTitle>
            <CardDescription className="font-mono">
              {profile.document.key}
            </CardDescription>
          </div>
          <Badge variant="secondary">{originLabels[profile.origin]}</Badge>
        </div>
      </CardHeader>
      <CardContent className="grid gap-3 text-sm">
        <div className="flex flex-wrap gap-2">
          <Badge variant="outline">{profileKindLabels[profile.document.kind]}</Badge>
          <Badge variant="outline">
            {accessPaths.length} Zugriffspfad
            {accessPaths.length === 1 ? "" : "e"}
          </Badge>
        </div>
        <div className="grid gap-2">
          {accessPaths.map((accessPath) => (
            <ProfileAccessPathRow
              key={accessPath.key}
              accessPath={accessPath}
              adapter={adaptersByKey.get(accessPath.adapterKey)}
            />
          ))}
        </div>
        <p className="break-all font-mono text-xs text-muted-foreground">
          {profile.path}
        </p>
      </CardContent>
    </Card>
  );
}

type ProfileAccessPathRowProps = {
  accessPath: ProfileAccessPathDefinition;
  adapter: AdapterMetadata | undefined;
};

function ProfileAccessPathRow({
  accessPath,
  adapter,
}: ProfileAccessPathRowProps) {
  const adapterDisplay = getAdapterDisplay(accessPath.adapterKey, adapter);

  return (
    <div className="rounded-lg border p-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="font-medium">{accessPath.name ?? accessPath.key}</p>
        <Badge variant={adapterDisplay.registered ? "secondary" : "warning-light"}>
          {adapterDisplay.registered ? "registriert" : "unregistriert"}
        </Badge>
      </div>
      <p className="mt-1 font-mono text-xs text-muted-foreground">
        {accessPath.key} · {adapterDisplay.key}
      </p>
    </div>
  );
}

type DiagnosticCardProps = {
  diagnostic: SourceRegistryDiagnostic;
};

function DiagnosticCard({ diagnostic }: DiagnosticCardProps) {
  return (
    <Card className="border-destructive/40">
      <CardHeader>
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <CardTitle className="text-base">
              {diagnosticCodeLabels[diagnostic.code]}
            </CardTitle>
            <CardDescription>
              {documentKindLabels[diagnostic.documentKind]} ·{" "}
              {originLabels[diagnostic.origin]}
            </CardDescription>
          </div>
          <Badge variant="destructive-light">{diagnostic.code}</Badge>
        </div>
      </CardHeader>
      <CardContent className="grid gap-2 text-sm">
        {diagnostic.key ? (
          <p>
            <span className="font-medium">Key:</span>{" "}
            <code>{diagnostic.key}</code>
          </p>
        ) : null}
        <p className="break-all">
          <span className="font-medium">Pfad:</span> {diagnostic.path}
        </p>
        <p>{diagnostic.message}</p>
      </CardContent>
    </Card>
  );
}

function sourceAdapterKey(
  source: RegistrySource,
  profilesByKey: Map<string, RegistrySourceProfile>,
) {
  const selectedAccessPath = source.document.selectedAccessPath;
  if (selectedAccessPath.type === "source_specific") {
    return selectedAccessPath.adapterKey;
  }

  const profile = profilesByKey.get(selectedAccessPath.profileKey);
  return profile?.document.accessPaths.find(
    (accessPath) => accessPath.key === selectedAccessPath.pathKey,
  )?.adapterKey;
}

function accessPathSummary(selectedAccessPath: SelectedAccessPath) {
  if (selectedAccessPath.type === "source_specific") {
    return `quellenspezifisch · ${selectedAccessPath.adapterKey}`;
  }

  return `Profil ${selectedAccessPath.profileKey} · Pfad ${selectedAccessPath.pathKey}`;
}

function sourceMatchesSearch(
  source: RegistrySource,
  search: string,
  profilesByKey: Map<string, RegistrySourceProfile>,
  adaptersByKey: Map<string, AdapterMetadata>,
) {
  const normalizedSearch = search.trim().toLocaleLowerCase("de");
  if (!normalizedSearch) return true;

  const adapterKey = sourceAdapterKey(source, profilesByKey);
  const adapter = adapterKey ? adaptersByKey.get(adapterKey) : null;
  const selectedAccessPath = source.document.selectedAccessPath;
  const haystack = [
    source.document.key,
    source.document.name,
    source.document.status,
    source.path,
    selectedAccessPath.type,
    selectedAccessPath.type === "profile" ? selectedAccessPath.profileKey : "",
    selectedAccessPath.type === "profile" ? selectedAccessPath.pathKey : "",
    adapterKey ?? "",
    adapter?.name ?? "",
  ]
    .join(" ")
    .toLocaleLowerCase("de");

  return haystack.includes(normalizedSearch);
}
