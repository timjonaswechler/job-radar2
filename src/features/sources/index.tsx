import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";

import {
  AlertCircleIcon,
  FileJsonIcon,
  PencilIcon,
  PlusIcon,
  RefreshCwIcon,
  Settings2Icon,
  Trash2Icon,
} from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import {
  DEFAULT_I18N,
  type Filter,
  type FilterFieldConfig,
  type FilterI18nConfig,
  Filters,
} from "@/components/reui/filters";
import {
  Frame,
  FrameDescription,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import {
  formatAdapterOptionLabel,
  getAdapterDisplay,
  sortAdaptersByUserFacingPriority,
} from "@/features/sources/adapter-metadata";
import { BrowserRuntimeCard } from "@/features/sources/components/browser-runtime-card";
import { BrowserProfileFormDialog } from "@/features/sources/components/browser-profile-form-dialog";
import { DeleteConfirmDialog } from "@/features/sources/components/delete-confirm-dialog";
import { SourceFormDialog } from "@/features/sources/components/source-form-dialog";
import { SystemProfileFormDialog } from "@/features/sources/components/system-profile-form-dialog";
import {
  sourceStatusBadgeVariants,
  sourceStatusLabels,
  sourceStatusOptions,
} from "@/features/sources/status";
import {
  createBrowserProfile,
  createSource,
  createSystemProfile,
  deleteBrowserProfile,
  deleteSource,
  deleteSystemProfile,
  exportSystemProfileJson,
  importSystemProfileJson,
  listAdapters,
  listBrowserProfiles,
  listSources,
  listSystemProfiles,
  testSystemProfileUrl,
  updateBrowserProfile,
  updateSource,
  updateSystemProfile,
  type AdapterMetadata,
  type BrowserProfile,
  type CreateBrowserProfileInput,
  type CreateSourceInput,
  type CreateSystemProfileInput,
  type JsonValue,
  type Source,
  type SourceStatus,
  type SystemProfile,
  type SystemProfileTestResult,
  type UpdateBrowserProfileInput,
  type UpdateSourceInput,
  type UpdateSystemProfileInput,
} from "@/lib/api/sources";
import { cn } from "@/lib/utils";

const filterI18n: FilterI18nConfig = {
  ...DEFAULT_I18N,
  addFilter: "Filter",
  addFilterTitle: "Filter hinzufügen",
  searchFields: "Filter suchen…",
  noFieldsFound: "Keine Filter gefunden.",
  noResultsFound: "Keine Ergebnisse gefunden.",
  select: "Auswählen…",
  selected: "ausgewählt",
  selectedCount: "ausgewählt",
  operators: {
    ...DEFAULT_I18N.operators,
    is: "ist",
    isNot: "ist nicht",
    isAnyOf: "ist eines von",
    isNotAnyOf: "ist keines von",
    contains: "enthält",
    notContains: "enthält nicht",
    empty: "ist leer",
    notEmpty: "ist nicht leer",
  },
  placeholders: {
    ...DEFAULT_I18N.placeholders,
    enterField: (fieldType: string) => `${fieldType} eingeben…`,
    selectField: "Auswählen…",
    searchField: (fieldName: string) => `${fieldName} durchsuchen…`,
    enterKey: "Key eingeben…",
    enterValue: "Wert eingeben…",
  },
};

const multiselectFilterOperators = [
  { value: "is_any_of", label: filterI18n.operators.isAnyOf },
  { value: "is_not_any_of", label: filterI18n.operators.isNotAnyOf },
];

type SourceInventory = {
  adapters: AdapterMetadata[];
  browserProfiles: BrowserProfile[];
  systemProfiles: SystemProfile[];
  sources: Source[];
};

type BrowserProfileDialogState =
  | { mode: "create" }
  | { mode: "edit"; browserProfile: BrowserProfile };

type SystemProfileDialogState =
  | { mode: "create" }
  | { mode: "edit"; systemProfile: SystemProfile };

type SourceDialogState =
  | { mode: "create" }
  | { mode: "edit"; source: Source };

type DeleteTarget =
  | { type: "browserProfile"; browserProfile: BrowserProfile }
  | { type: "systemProfile"; systemProfile: SystemProfile }
  | { type: "source"; source: Source };

function useSourceInventory() {
  const [data, setData] = useState<SourceInventory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [browserProfiles, systemProfiles, sources, adapters] = await Promise.all([
        listBrowserProfiles(),
        listSystemProfiles(),
        listSources(),
        listAdapters(),
      ]);
      setData({ adapters, browserProfiles, systemProfiles, sources });
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
  const { data, error, loading, refresh } = useSourceInventory();
  const [filters, setFilters] = useState<Filter<string>[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState<number | null>(null);
  const [browserProfileDialog, setBrowserProfileDialog] =
    useState<BrowserProfileDialogState | null>(null);
  const [systemProfileDialog, setSystemProfileDialog] =
    useState<SystemProfileDialogState | null>(null);
  const [sourceDialog, setSourceDialog] = useState<SourceDialogState | null>(
    null,
  );
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null);
  const systemProfileImportInputRef = useRef<HTMLInputElement | null>(null);

  const adapters = data?.adapters ?? [];
  const browserProfiles = data?.browserProfiles ?? [];
  const systemProfiles = data?.systemProfiles ?? [];
  const sources = data?.sources ?? [];

  const browserProfilesById = useMemo(
    () => new Map(browserProfiles.map((browserProfile) => [browserProfile.id, browserProfile])),
    [browserProfiles],
  );

  const systemProfilesById = useMemo(
    () => new Map(systemProfiles.map((systemProfile) => [systemProfile.id, systemProfile])),
    [systemProfiles],
  );

  const adaptersByKey = useMemo(
    () => new Map(adapters.map((adapter) => [adapter.key, adapter])),
    [adapters],
  );

  const adapterOptions = useMemo(() => {
    const registeredOptions = sortAdaptersByUserFacingPriority(adapters).map((adapter) => ({
      value: adapter.key,
      label: formatAdapterOptionLabel(adapter),
    }));
    const fallbackOptions = Array.from(
      new Set(sources.map((source) => source.adapterKey)),
    )
      .filter((adapterKey) => !adaptersByKey.has(adapterKey))
      .sort()
      .map((adapterKey) => ({
        value: adapterKey,
        label: `${adapterKey} (unregistriert)`,
      }));

    return [...registeredOptions, ...fallbackOptions];
  }, [adapters, adaptersByKey, sources]);

  const filterFields = useMemo<FilterFieldConfig<string>[]>(
    () => [
      {
        key: "search",
        label: "Suche",
        type: "text",
        defaultOperator: "contains",
        operators: [
          { value: "contains", label: "enthält" },
          { value: "not_contains", label: "enthält nicht" },
        ],
        placeholder: "Name, Key, Adapter oder Beschreibung",
      },
      {
        key: "status",
        label: "Status",
        type: "multiselect",
        operators: multiselectFilterOperators,
        options: sourceStatusOptions,
      },
      {
        key: "adapter",
        label: "Adapter",
        type: "multiselect",
        operators: multiselectFilterOperators,
        options: adapterOptions,
      },
      {
        key: "browserProfile",
        label: "Browserprofil",
        type: "multiselect",
        operators: multiselectFilterOperators,
        options: browserProfiles.map((browserProfile) => ({
          value: String(browserProfile.id),
          label: browserProfile.name,
        })),
      },
    ],
    [adapterOptions, browserProfiles],
  );

  const filteredSources = useMemo(
    () =>
      sources.filter((source) =>
        filters.every((filter) =>
          sourceMatchesFilter(source, filter, browserProfilesById, adaptersByKey),
        ),
      ),
    [adaptersByKey, browserProfilesById, filters, sources],
  );

  useEffect(() => {
    if (!filteredSources.length) {
      setSelectedSourceId(null);
      return;
    }

    if (!filteredSources.some((source) => source.id === selectedSourceId)) {
      setSelectedSourceId(filteredSources[0].id);
    }
  }, [filteredSources, selectedSourceId]);

  const selectedSource =
    filteredSources.find((source) => source.id === selectedSourceId) ?? null;

  const handleCreateBrowserProfile = async (
    input: CreateBrowserProfileInput,
  ) => {
    const created = await createBrowserProfile(input);
    toast.success("Browserprofil angelegt.", { description: created.name });
    await refresh();
  };

  const handleUpdateBrowserProfile = async (
    browserProfile: BrowserProfile,
    input: UpdateBrowserProfileInput,
  ) => {
    const updated = await updateBrowserProfile(browserProfile.id, input);
    toast.success("Browserprofil gespeichert.", { description: updated.name });
    await refresh();
  };

  const handleCreateSystemProfile = async (
    input: CreateSystemProfileInput,
  ) => {
    const created = await createSystemProfile(input);
    toast.success("Systemprofil angelegt.", { description: created.name });
    await refresh();
  };

  const handleUpdateSystemProfile = async (
    systemProfile: SystemProfile,
    input: UpdateSystemProfileInput,
  ) => {
    const updated = await updateSystemProfile(systemProfile.id, input);
    toast.success("Systemprofil gespeichert.", { description: updated.name });
    await refresh();
  };

  const handleExportSystemProfile = async (systemProfile: SystemProfile) => {
    try {
      const json = await exportSystemProfileJson(systemProfile.id);
      downloadJsonFile(`${systemProfile.key}.json`, json);
      toast.success("Systemprofil exportiert.", {
        description: systemProfile.name,
      });
    } catch (unknownError) {
      toast.error("Systemprofil konnte nicht exportiert werden.", {
        description: String(unknownError),
      });
    }
  };

  const handleImportSystemProfileFile = async (
    event: ChangeEvent<HTMLInputElement>,
  ) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    try {
      const imported = await importSystemProfileJson(await file.text());
      toast.success("Systemprofil importiert.", { description: imported.name });
      await refresh();
    } catch (unknownError) {
      toast.error("Systemprofil konnte nicht importiert werden.", {
        description: String(unknownError),
      });
    }
  };

  const handleCreateSource = async (input: CreateSourceInput) => {
    const created = await createSource(input);
    toast.success("Quelle angelegt.", { description: created.name });
    await refresh();
    setSelectedSourceId(created.id);
  };

  const handleUpdateSource = async (
    source: Source,
    input: UpdateSourceInput,
  ) => {
    const updated = await updateSource(source.id, input);
    toast.success("Quelle gespeichert.", { description: updated.name });
    await refresh();
    setSelectedSourceId(updated.id);
  };

  const handleConfirmDelete = async () => {
    if (!deleteTarget) return;

    if (deleteTarget.type === "browserProfile") {
      await deleteBrowserProfile(deleteTarget.browserProfile.id);
      toast.success("Browserprofil gelöscht.", {
        description: deleteTarget.browserProfile.name,
      });
      await refresh();
      return;
    }

    if (deleteTarget.type === "systemProfile") {
      await deleteSystemProfile(deleteTarget.systemProfile.id);
      toast.success("Systemprofil gelöscht.", {
        description: deleteTarget.systemProfile.name,
      });
      await refresh();
      return;
    }

    await deleteSource(deleteTarget.source.id);
    toast.success("Quelle gelöscht.", { description: deleteTarget.source.name });
    if (selectedSourceId === deleteTarget.source.id) setSelectedSourceId(null);
    await refresh();
  };

  const deleteDialogCopy = getDeleteDialogCopy(deleteTarget);

  return (
    <div className="grid gap-5">
      <Frame>
        <FramePanel>
          <FrameHeader className="gap-4 sm:flex-row sm:items-start sm:justify-between">
            <div className="grid gap-1.5">
              <FrameTitle>Quellen</FrameTitle>
              <FrameDescription>
                Browser-Laufzeit, Browserprofile und Quellen aktiv konfigurieren.
                Suchkriterien gehören später in Suchanfragen.
              </FrameDescription>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button variant="outline" size="sm" onClick={() => void refresh()}>
                <RefreshCwIcon className="size-4" aria-hidden="true" />
                Aktualisieren
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => systemProfileImportInputRef.current?.click()}
              >
                <FileJsonIcon className="size-4" aria-hidden="true" />
                Systemprofil importieren
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setSystemProfileDialog({ mode: "create" })}
              >
                <PlusIcon className="size-4" aria-hidden="true" />
                Systemprofil
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setBrowserProfileDialog({ mode: "create" })}
              >
                <PlusIcon className="size-4" aria-hidden="true" />
                Browserprofil
              </Button>
              <Button size="sm" onClick={() => setSourceDialog({ mode: "create" })}>
                <PlusIcon className="size-4" aria-hidden="true" />
                Neue Quelle
              </Button>
            </div>
          </FrameHeader>

          <Alert variant="info">
            <Settings2Icon className="size-4" aria-hidden="true" />
            <AlertTitle>Quellen enthalten keine Suchkriterien</AlertTitle>
            <AlertDescription>
              Eine Quelle beschreibt stabile Zugangsparameter und den Adapter.
              Keywords, Ort, Region und weitere Filter gehören in spätere
              Suchanfragen.
            </AlertDescription>
          </Alert>
        </FramePanel>
      </Frame>

      <BrowserRuntimeCard />

      <SystemProfileTestRunner
        systemProfiles={systemProfiles}
        loading={loading}
      />

      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Quellen konnten nicht geladen werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <Frame>
        <FramePanel>
          <FrameHeader className="gap-2">
            <FrameTitle>Filter</FrameTitle>
            <FrameDescription>
              Quellen nach Status, Adapter oder Browserprofil eingrenzen.
            </FrameDescription>
          </FrameHeader>
          <Filters
            filters={filters}
            fields={filterFields}
            onChange={setFilters}
            allowMultiple={false}
            i18n={filterI18n}
            size="sm"
          />
        </FramePanel>
      </Frame>

      <div className="grid gap-4 xl:grid-cols-[minmax(18rem,0.9fr)_minmax(0,1.15fr)_minmax(20rem,1fr)]">
        <ProfileCatalog
          browserProfiles={browserProfiles}
          systemProfiles={systemProfiles}
          loading={loading}
          onCreateBrowserProfile={() => setBrowserProfileDialog({ mode: "create" })}
          onEditBrowserProfile={(browserProfile) =>
            setBrowserProfileDialog({ mode: "edit", browserProfile })
          }
          onDeleteBrowserProfile={(browserProfile) =>
            setDeleteTarget({ type: "browserProfile", browserProfile })
          }
          onCreateSystemProfile={() => setSystemProfileDialog({ mode: "create" })}
          onImportSystemProfile={() => systemProfileImportInputRef.current?.click()}
          onExportSystemProfile={(systemProfile) =>
            void handleExportSystemProfile(systemProfile)
          }
          onEditSystemProfile={(systemProfile) =>
            setSystemProfileDialog({ mode: "edit", systemProfile })
          }
          onDeleteSystemProfile={(systemProfile) =>
            setDeleteTarget({ type: "systemProfile", systemProfile })
          }
        />

        <SourcesList
          sources={filteredSources}
          loading={loading}
          selectedSourceId={selectedSourceId}
          browserProfilesById={browserProfilesById}
          systemProfilesById={systemProfilesById}
          adaptersByKey={adaptersByKey}
          onCreate={() => setSourceDialog({ mode: "create" })}
          onSelect={setSelectedSourceId}
        />

        <SourceDetails
          source={selectedSource}
          browserProfile={
            selectedSource?.browserProfileId
              ? browserProfilesById.get(selectedSource.browserProfileId) ?? null
              : null
          }
          systemProfile={
            selectedSource?.systemProfileId
              ? systemProfilesById.get(selectedSource.systemProfileId) ?? null
              : null
          }
          loading={loading}
          adaptersByKey={adaptersByKey}
          onEdit={(source) => setSourceDialog({ mode: "edit", source })}
          onDelete={(source) => setDeleteTarget({ type: "source", source })}
        />
      </div>

      <input
        ref={systemProfileImportInputRef}
        type="file"
        accept="application/json,.json"
        className="hidden"
        onChange={(event) => void handleImportSystemProfileFile(event)}
      />

      {browserProfileDialog?.mode === "create" ? (
        <BrowserProfileFormDialog
          open
          mode="create"
          onOpenChange={(open) => {
            if (!open) setBrowserProfileDialog(null);
          }}
          onSubmit={handleCreateBrowserProfile}
        />
      ) : null}

      {browserProfileDialog?.mode === "edit" ? (
        <BrowserProfileFormDialog
          open
          mode="edit"
          browserProfile={browserProfileDialog.browserProfile}
          onOpenChange={(open) => {
            if (!open) setBrowserProfileDialog(null);
          }}
          onSubmit={(input) =>
            handleUpdateBrowserProfile(
              browserProfileDialog.browserProfile,
              input,
            )
          }
        />
      ) : null}

      {systemProfileDialog?.mode === "create" ? (
        <SystemProfileFormDialog
          open
          mode="create"
          adapters={adapters}
          onOpenChange={(open) => {
            if (!open) setSystemProfileDialog(null);
          }}
          onSubmit={handleCreateSystemProfile}
        />
      ) : null}

      {systemProfileDialog?.mode === "edit" ? (
        <SystemProfileFormDialog
          open
          mode="edit"
          systemProfile={systemProfileDialog.systemProfile}
          adapters={adapters}
          onOpenChange={(open) => {
            if (!open) setSystemProfileDialog(null);
          }}
          onSubmit={(input) =>
            handleUpdateSystemProfile(
              systemProfileDialog.systemProfile,
              input,
            )
          }
        />
      ) : null}

      {sourceDialog?.mode === "create" ? (
        <SourceFormDialog
          open
          mode="create"
          browserProfiles={browserProfiles}
          systemProfiles={systemProfiles}
          adapters={adapters}
          onOpenChange={(open) => {
            if (!open) setSourceDialog(null);
          }}
          onSubmit={handleCreateSource}
        />
      ) : null}

      {sourceDialog?.mode === "edit" ? (
        <SourceFormDialog
          open
          mode="edit"
          source={sourceDialog.source}
          browserProfiles={browserProfiles}
          systemProfiles={systemProfiles}
          adapters={adapters}
          onOpenChange={(open) => {
            if (!open) setSourceDialog(null);
          }}
          onSubmit={(input) => handleUpdateSource(sourceDialog.source, input)}
        />
      ) : null}

      <DeleteConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        title={deleteDialogCopy.title}
        description={deleteDialogCopy.description}
        confirmLabel="Endgültig löschen"
        onConfirm={handleConfirmDelete}
      />
    </div>
  );
}

function SystemProfileTestRunner({
  systemProfiles,
  loading,
}: {
  systemProfiles: SystemProfile[];
  loading: boolean;
}) {
  const [url, setUrl] = useState("");
  const [systemProfileId, setSystemProfileId] = useState("");
  const [result, setResult] = useState<SystemProfileTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    if (!systemProfiles.length) {
      if (systemProfileId) setSystemProfileId("");
      return;
    }

    if (
      !systemProfiles.some(
        (systemProfile) => String(systemProfile.id) === systemProfileId,
      )
    ) {
      setSystemProfileId(String(systemProfiles[0].id));
    }
  }, [systemProfileId, systemProfiles]);

  const selectedSystemProfile = useMemo(() => {
    if (!systemProfileId) return null;
    return (
      systemProfiles.find(
        (systemProfile) => systemProfile.id === Number(systemProfileId),
      ) ?? null
    );
  }, [systemProfileId, systemProfiles]);

  const handleRun = async () => {
    setError(null);
    setResult(null);

    if (!selectedSystemProfile) {
      setError("Bitte ein Systemprofil auswählen.");
      return;
    }

    try {
      setRunning(true);
      const nextResult = await testSystemProfileUrl(
        url,
        selectedSystemProfile.id,
      );
      setResult(nextResult);
    } catch (unknownError) {
      setError(String(unknownError));
    } finally {
      setRunning(false);
    }
  };

  return (
    <Frame>
      <FramePanel>
        <FrameHeader className="gap-2 px-0 pt-0">
          <FrameTitle>Systemprofil testen</FrameTitle>
          <FrameDescription>
            Prüft eine URL gezielt gegen genau ein Systemprofil. Der Lauf zeigt
            Pflichtcheck-Evidence und den Quellenvorschlag, legt aber keine
            Quelle an und speichert nichts automatisch.
          </FrameDescription>
        </FrameHeader>

        <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_minmax(14rem,0.7fr)_auto] md:items-end">
          <div className="grid gap-1.5">
            <label
              className="text-xs font-medium"
              htmlFor="system-profile-test-url"
            >
              URL
            </label>
            <Input
              id="system-profile-test-url"
              value={url}
              onChange={(event) => {
                setUrl(event.target.value);
                setError(null);
                setResult(null);
              }}
              disabled={running}
              placeholder="https://example.com/jobs"
            />
          </div>
          <div className="grid gap-1.5">
            <label
              className="text-xs font-medium"
              htmlFor="system-profile-test-profile"
            >
              Systemprofil
            </label>
            <NativeSelect
              id="system-profile-test-profile"
              className="w-full"
              value={systemProfileId}
              onChange={(event) => {
                setSystemProfileId(event.target.value);
                setError(null);
                setResult(null);
              }}
              disabled={loading || running || !systemProfiles.length}
            >
              <NativeSelectOption value="" disabled>
                Systemprofil wählen
              </NativeSelectOption>
              {systemProfiles.map((systemProfile) => (
                <NativeSelectOption
                  key={systemProfile.id}
                  value={String(systemProfile.id)}
                >
                  {systemProfile.name} ({systemProfile.key},{" "}
                  {sourceStatusLabels[systemProfile.status]})
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </div>
          <Button
            type="button"
            onClick={() => void handleRun()}
            disabled={loading || running || !systemProfiles.length}
          >
            {running ? "Prüft…" : "Profil prüfen"}
          </Button>
        </div>

        {!loading && !systemProfiles.length ? (
          <p className="mt-3 text-sm text-muted-foreground">
            Noch keine Systemprofile vorhanden.
          </p>
        ) : null}

        {error ? (
          <Alert className="mt-3" variant="destructive">
            <AlertCircleIcon className="size-4" aria-hidden="true" />
            <AlertTitle>Profilcheck konnte nicht ausgeführt werden</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {result ? <SystemProfileTestResultView result={result} /> : null}
      </FramePanel>
    </Frame>
  );
}

function SystemProfileTestResultView({
  result,
}: {
  result: SystemProfileTestResult;
}) {
  const passed = result.status === "passed";

  return (
    <div className="mt-4 grid gap-3">
      <Alert variant={passed ? "success" : "warning"}>
        <AlertCircleIcon className="size-4" aria-hidden="true" />
        <AlertTitle>
          {passed ? "Profilcheck bestanden" : "Profilcheck fehlgeschlagen"}
        </AlertTitle>
        <AlertDescription>
          {passed
            ? `Alle Pflichtchecks für ${result.systemProfileName} wurden erfüllt.`
            : `Mindestens ein Pflichtcheck für ${result.systemProfileName} ist fehlgeschlagen.`}
        </AlertDescription>
      </Alert>

      <section className="grid gap-2">
        <h3 className="text-sm font-medium">Pflichtchecks</h3>
        <div className="grid gap-2">
          {result.checks.map((check) => (
            <div key={check.index} className="grid gap-2 rounded-md border p-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-sm font-medium">
                  Pflichtcheck {check.index}
                </span>
                <Badge
                  variant={
                    check.status === "passed"
                      ? "success-light"
                      : "destructive-light"
                  }
                  size="sm"
                >
                  {check.status === "passed" ? "Bestanden" : "Fehlgeschlagen"}
                </Badge>
              </div>
              <p
                className={cn(
                  "text-xs",
                  check.status === "passed"
                    ? "text-muted-foreground"
                    : "text-destructive",
                )}
              >
                {check.evidence ?? check.diagnostic}
              </p>
              <pre className="max-h-40 overflow-auto rounded-md bg-muted/40 p-2 text-xs text-muted-foreground">
                {formatJson(check.check)}
              </pre>
            </div>
          ))}
        </div>
      </section>

      {!passed ? (
        <p className="text-sm text-muted-foreground">
          Keine Quellenkonfiguration wird vorgeschlagen, solange ein Pflichtcheck
          fehlschlägt.
        </p>
      ) : null}

      {passed && result.sourceConfig ? (
        <section className="grid gap-2">
          <h3 className="text-sm font-medium">
            Vorgeschlagene Quellenkonfiguration
          </h3>
          <div className="grid gap-2 rounded-md border p-3 text-sm">
            {result.name ? <DetailRow label="Name" value={result.name} /> : null}
            {result.key ? <DetailRow label="Key" value={result.key} /> : null}
            <DetailRow label="Adapter" value={result.adapterKey} />
            <DetailRow label="Systemprofil" value={result.systemProfileName} />
            <pre className="max-h-72 overflow-auto rounded-md bg-muted/40 p-3 text-xs text-muted-foreground">
              {formatJson(result.sourceConfig)}
            </pre>
          </div>
        </section>
      ) : null}
    </div>
  );
}

function ProfileCatalog({
  browserProfiles,
  systemProfiles,
  loading,
  onCreateBrowserProfile,
  onEditBrowserProfile,
  onDeleteBrowserProfile,
  onCreateSystemProfile,
  onImportSystemProfile,
  onExportSystemProfile,
  onEditSystemProfile,
  onDeleteSystemProfile,
}: {
  browserProfiles: BrowserProfile[];
  systemProfiles: SystemProfile[];
  loading: boolean;
  onCreateBrowserProfile: () => void;
  onEditBrowserProfile: (browserProfile: BrowserProfile) => void;
  onDeleteBrowserProfile: (browserProfile: BrowserProfile) => void;
  onCreateSystemProfile: () => void;
  onImportSystemProfile: () => void;
  onExportSystemProfile: (systemProfile: SystemProfile) => void;
  onEditSystemProfile: (systemProfile: SystemProfile) => void;
  onDeleteSystemProfile: (systemProfile: SystemProfile) => void;
}) {
  return (
    <Frame className="content-start" spacing="sm">
      <FramePanel>
        <FrameHeader className="gap-3 px-0 pt-0 sm:flex-row sm:items-start sm:justify-between">
          <div className="grid gap-1">
            <FrameTitle>Katalog</FrameTitle>
            <FrameDescription>
              Systemprofile erkennen Recruiting-Systeme; Browserprofile steuern
              browserbasierte Laufzeiten.
            </FrameDescription>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button type="button" variant="outline" size="sm" onClick={onImportSystemProfile}>
              <FileJsonIcon className="size-4" aria-hidden="true" />
              Import
            </Button>
            <Button type="button" variant="outline" size="sm" onClick={onCreateSystemProfile}>
              <PlusIcon className="size-4" aria-hidden="true" />
              System
            </Button>
            <Button type="button" variant="outline" size="sm" onClick={onCreateBrowserProfile}>
              <PlusIcon className="size-4" aria-hidden="true" />
              Browser
            </Button>
          </div>
        </FrameHeader>

        <section className="grid gap-3">
          <CatalogSectionTitle
            label="Systemprofile"
            count={systemProfiles.length}
          />
          {loading ? (
            <MutedLine>Lade Systemprofile…</MutedLine>
          ) : systemProfiles.length ? (
            <div className="grid gap-2">
              {systemProfiles.map((systemProfile) => (
                <div
                  key={systemProfile.id}
                  className="grid gap-3 rounded-md border bg-muted/30 p-3"
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="truncate text-sm font-medium">
                        {systemProfile.name}
                      </div>
                      <div className="mt-0.5 truncate text-xs text-muted-foreground">
                        {systemProfile.key} · {systemProfile.adapterKey}
                        {systemProfile.builtIn ? " · Eingebaut" : ""}
                      </div>
                    </div>
                    <StatusBadge status={systemProfile.status} />
                  </div>
                  {systemProfile.validationError ? (
                    <p className="line-clamp-2 text-xs text-destructive">
                      {systemProfile.validationError}
                    </p>
                  ) : null}
                  <div className="flex flex-wrap gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => onExportSystemProfile(systemProfile)}
                    >
                      <FileJsonIcon className="size-3" aria-hidden="true" />
                      Export
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => onEditSystemProfile(systemProfile)}
                      disabled={systemProfile.builtIn}
                    >
                      <PencilIcon className="size-3" aria-hidden="true" />
                      Bearbeiten
                    </Button>
                    <Button
                      type="button"
                      variant="destructive"
                      size="sm"
                      onClick={() => onDeleteSystemProfile(systemProfile)}
                      disabled={systemProfile.builtIn}
                    >
                      <Trash2Icon className="size-3" aria-hidden="true" />
                      Löschen
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="rounded-md border border-dashed p-4 text-center">
              <p className="text-sm font-medium">
                Noch keine Systemprofile registriert.
              </p>
              <Button
                type="button"
                size="sm"
                className="mt-3"
                onClick={onCreateSystemProfile}
              >
                <PlusIcon className="size-4" aria-hidden="true" />
                Systemprofil anlegen
              </Button>
            </div>
          )}

          <CatalogSectionTitle
            label="Browserprofile"
            count={browserProfiles.length}
          />
          {loading ? (
            <MutedLine>Lade Browserprofile…</MutedLine>
          ) : browserProfiles.length ? (
            <div className="grid gap-2">
              {browserProfiles.map((browserProfile) => (
                <div
                  key={browserProfile.id}
                  className="grid gap-3 rounded-md border bg-muted/30 p-3"
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="truncate text-sm font-medium">
                        {browserProfile.name}
                      </div>
                      <div className="mt-0.5 truncate text-xs text-muted-foreground">
                        {browserProfile.key} · Schema v
                        {browserProfile.definitionSchemaVersion}
                      </div>
                    </div>
                    <StatusBadge status={browserProfile.status} />
                  </div>
                  {browserProfile.validationError ? (
                    <p className="line-clamp-2 text-xs text-destructive">
                      {browserProfile.validationError}
                    </p>
                  ) : null}
                  <div className="flex flex-wrap gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => onEditBrowserProfile(browserProfile)}
                    >
                      <PencilIcon className="size-3" aria-hidden="true" />
                      Bearbeiten
                    </Button>
                    <Button
                      type="button"
                      variant="destructive"
                      size="sm"
                      onClick={() => onDeleteBrowserProfile(browserProfile)}
                    >
                      <Trash2Icon className="size-3" aria-hidden="true" />
                      Löschen
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="rounded-md border border-dashed p-4 text-center">
              <p className="text-sm font-medium">
                Noch keine Browserprofile registriert.
              </p>
              <Button
                type="button"
                size="sm"
                className="mt-3"
                onClick={onCreateBrowserProfile}
              >
                <PlusIcon className="size-4" aria-hidden="true" />
                Browserprofil anlegen
              </Button>
            </div>
          )}
        </section>
      </FramePanel>
    </Frame>
  );
}

function SourcesList({
  sources,
  loading,
  selectedSourceId,
  browserProfilesById,
  systemProfilesById,
  adaptersByKey,
  onCreate,
  onSelect,
}: {
  sources: Source[];
  loading: boolean;
  selectedSourceId: number | null;
  browserProfilesById: Map<number, BrowserProfile>;
  systemProfilesById: Map<number, SystemProfile>;
  adaptersByKey: Map<string, AdapterMetadata>;
  onCreate: () => void;
  onSelect: (id: number) => void;
}) {
  return (
    <Frame>
      <FramePanel>
        <FrameHeader className="gap-3 px-0 pt-0 sm:flex-row sm:items-start sm:justify-between">
          <div className="grid gap-1">
            <FrameTitle>Quellen</FrameTitle>
            <FrameDescription>
              Gespeicherte Herkunfts- und Zugriffskonfigurationen.
            </FrameDescription>
          </div>
          <Button type="button" size="sm" onClick={onCreate}>
            <PlusIcon className="size-4" aria-hidden="true" />
            Neu
          </Button>
        </FrameHeader>

        {loading ? (
          <MutedLine>Lade Quellen…</MutedLine>
        ) : sources.length ? (
          <div className="overflow-hidden rounded-md border">
            <div className="grid grid-cols-[minmax(0,1.3fr)_10rem_8rem] gap-3 border-b bg-muted/50 px-3 py-2 text-xs font-medium text-muted-foreground max-md:hidden">
              <div>Quelle</div>
              <div>Adapter</div>
              <div>Status</div>
            </div>
            <div className="divide-y">
              {sources.map((source) => {
                const browserProfile = source.browserProfileId
                  ? browserProfilesById.get(source.browserProfileId)
                  : null;
                const systemProfile = source.systemProfileId
                  ? systemProfilesById.get(source.systemProfileId)
                  : null;
                const adapterDisplay = getAdapterDisplay(
                  source.adapterKey,
                  adaptersByKey.get(source.adapterKey),
                );

                return (
                  <button
                    key={source.id}
                    type="button"
                    className={cn(
                      "grid w-full gap-3 px-3 py-3 text-left transition-colors hover:bg-muted/60 md:grid-cols-[minmax(0,1.3fr)_10rem_8rem]",
                      selectedSourceId === source.id && "bg-muted",
                    )}
                    onClick={() => onSelect(source.id)}
                  >
                    <div className="min-w-0">
                      <div className="truncate text-sm font-medium">
                        {source.name}
                      </div>
                      <div className="mt-0.5 truncate text-xs text-muted-foreground">
                        {source.key}
                        {source.builtIn ? " · Eingebaut" : ""}
                        {systemProfile ? ` · ${systemProfile.name}` : ""}
                        {browserProfile ? ` · ${browserProfile.name}` : ""}
                      </div>
                    </div>
                    <div className="min-w-0 self-center">
                      <div className="truncate text-xs font-medium">
                        {adapterDisplay.name}
                      </div>
                      <div className="mt-0.5 truncate text-xs text-muted-foreground">
                        {adapterDisplay.key}
                      </div>
                    </div>
                    <div className="self-center">
                      <StatusBadge status={source.status} />
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        ) : (
          <div className="rounded-md border border-dashed p-6 text-center">
            <p className="text-sm font-medium">Noch keine Quellen vorhanden.</p>
            <p className="mt-1 text-sm text-muted-foreground">
              Lege eine Quelle mit stabilem Adapter- und Zugriffskontext an.
            </p>
            <Button type="button" size="sm" className="mt-3" onClick={onCreate}>
              <PlusIcon className="size-4" aria-hidden="true" />
              Quelle anlegen
            </Button>
          </div>
        )}
      </FramePanel>
    </Frame>
  );
}

function SourceDetails({
  source,
  browserProfile,
  systemProfile,
  loading,
  adaptersByKey,
  onEdit,
  onDelete,
}: {
  source: Source | null;
  browserProfile: BrowserProfile | null;
  systemProfile: SystemProfile | null;
  loading: boolean;
  adaptersByKey: Map<string, AdapterMetadata>;
  onEdit: (source: Source) => void;
  onDelete: (source: Source) => void;
}) {
  const adapterDisplay = source
    ? getAdapterDisplay(source.adapterKey, adaptersByKey.get(source.adapterKey))
    : null;

  return (
    <Frame>
      <FramePanel>
        <FrameHeader className="gap-3 px-0 pt-0 sm:flex-row sm:items-start sm:justify-between">
          <div className="grid gap-1">
            <FrameTitle>Details</FrameTitle>
            <FrameDescription>
              Adapter, Browserprofil und Quellenkonfiguration.
            </FrameDescription>
          </div>
          {source ? (
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => onEdit(source)}
              >
                <PencilIcon className="size-4" aria-hidden="true" />
                Bearbeiten
              </Button>
              <Button
                type="button"
                variant="destructive"
                size="sm"
                onClick={() => onDelete(source)}
                disabled={source.builtIn}
                title={source.builtIn ? "Eingebaute Job-Portale können nicht gelöscht werden." : undefined}
              >
                <Trash2Icon className="size-4" aria-hidden="true" />
                Löschen
              </Button>
            </div>
          ) : null}
        </FrameHeader>

        {loading ? (
          <MutedLine>Lade Details…</MutedLine>
        ) : source ? (
          <div className="grid gap-4">
            <div className="flex flex-wrap items-center gap-2">
              <StatusBadge status={source.status} />
              <Badge variant="info-light" size="sm">
                {adapterDisplay?.name}
              </Badge>
            </div>

            {source.validationError ? (
              <Alert variant="warning">
                <AlertCircleIcon className="size-4" aria-hidden="true" />
                <AlertTitle>Validierungsfehler</AlertTitle>
                <AlertDescription>{source.validationError}</AlertDescription>
              </Alert>
            ) : null}

            <dl className="grid gap-3 text-sm">
              <DetailRow label="Name" value={source.name} />
              <DetailRow label="Key" value={source.key} />
              {source.builtIn ? (
                <DetailRow label="Typ" value="Eingebautes Job-Portal" />
              ) : null}
              <DetailRow label="Adapter" value={adapterDisplay?.name ?? source.adapterKey} />
              <DetailRow label="Adapter-Key" value={source.adapterKey} />
              <DetailRow
                label="Systemprofil"
                value={systemProfile?.name ?? "Kein Systemprofil"}
              />
              <DetailRow
                label="Browserprofil"
                value={browserProfile?.name ?? "Kein Browserprofil"}
              />
              {source.description ? (
                <DetailRow label="Beschreibung" value={source.description} />
              ) : null}
              <DetailRow label="Aktualisiert" value={source.updatedAt} />
            </dl>

            <div className="grid gap-2">
              <div className="flex items-center gap-2 text-sm font-medium">
                <FileJsonIcon className="size-4 text-muted-foreground" />
                Quellenkonfiguration JSON
              </div>
              <pre className="max-h-72 overflow-auto rounded-md border bg-muted/40 p-3 text-xs text-muted-foreground">
                {formatJson(source.sourceConfig)}
              </pre>
            </div>
          </div>
        ) : (
          <div className="rounded-md border border-dashed p-6 text-center">
            <p className="text-sm font-medium">Keine Quelle ausgewählt.</p>
            <p className="mt-1 text-sm text-muted-foreground">
              Wähle links eine Quelle aus, sobald Daten vorhanden sind.
            </p>
          </div>
        )}
      </FramePanel>
    </Frame>
  );
}

function CatalogSectionTitle({ label, count }: { label: string; count: number }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <h2 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </h2>
      <Badge variant="outline" size="xs">
        {count}
      </Badge>
    </div>
  );
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md bg-muted/40 p-3">
      <dt className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </dt>
      <dd className="mt-1 break-words">{value}</dd>
    </div>
  );
}

function MutedLine({ children }: { children: string }) {
  return <p className="text-sm text-muted-foreground">{children}</p>;
}

function StatusBadge({ status }: { status: SourceStatus }) {
  return (
    <Badge variant={sourceStatusBadgeVariants[status]} size="sm">
      {sourceStatusLabels[status]}
    </Badge>
  );
}

function sourceMatchesFilter(
  source: Source,
  filter: Filter<string>,
  browserProfilesById: Map<number, BrowserProfile>,
  adaptersByKey: Map<string, AdapterMetadata>,
) {
  const values = filter.values.filter(Boolean);

  if (!values.length) return true;

  if (filter.field === "search") {
    const needle = values[0].toLowerCase();
    const browserProfile = source.browserProfileId
      ? browserProfilesById.get(source.browserProfileId)
      : null;
    const adapter = adaptersByKey.get(source.adapterKey);
    const haystack = [
      source.name,
      source.key,
      source.description,
      source.adapterKey,
      adapter?.name,
      browserProfile?.name,
      browserProfile?.key,
    ]
      .filter(Boolean)
      .join(" ")
      .toLowerCase();
    const matches = haystack.includes(needle);
    return filter.operator === "not_contains" ? !matches : matches;
  }

  const actualValue = getFilterValue(source, filter.field);
  const matches = actualValue ? values.includes(actualValue) : false;

  return filter.operator.includes("not") ? !matches : matches;
}

function getFilterValue(source: Source, field: string) {
  if (field === "status") return source.status;
  if (field === "adapter") return source.adapterKey;
  if (field === "browserProfile") {
    return source.browserProfileId ? String(source.browserProfileId) : null;
  }
  return null;
}

function formatJson(value: JsonValue) {
  return JSON.stringify(value, null, 2);
}

function downloadJsonFile(filename: string, contents: string) {
  const blob = new Blob([contents], { type: "application/json;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

function getDeleteDialogCopy(deleteTarget: DeleteTarget | null) {
  if (!deleteTarget) {
    return { title: "Löschen", description: "Dieses Element löschen?" };
  }

  if (deleteTarget.type === "browserProfile") {
    return {
      title: `Browserprofil „${deleteTarget.browserProfile.name}“ löschen?`,
      description:
        "Das Browserprofil wird endgültig gelöscht. Quellen, die dieses Profil verwenden, verhindern das Löschen über die Datenbank-Referenz.",
    };
  }

  if (deleteTarget.type === "systemProfile") {
    return {
      title: `Systemprofil „${deleteTarget.systemProfile.name}“ löschen?`,
      description:
        "Das Systemprofil wird endgültig gelöscht. Quellen, die dieses Profil verwenden, verhindern das Löschen über die Datenbank-Referenz.",
    };
  }

  return {
    title: `Quelle „${deleteTarget.source.name}“ löschen?`,
    description:
      "Die Quelle wird endgültig gelöscht. Suchkriterien sind davon nicht betroffen, weil sie nicht Teil der Quelle sind.",
  };
}
