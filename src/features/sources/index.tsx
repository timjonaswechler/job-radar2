import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type FormEvent,
} from "react";

import {
  AlertCircleIcon,
  FileJsonIcon,
  GripVerticalIcon,
  MoreHorizontalIcon,
  PencilIcon,
  PlusIcon,
  RefreshCwIcon,
  Trash2Icon,
} from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import {
  Kanban,
  KanbanBoard,
  KanbanColumn,
  KanbanColumnContent,
  KanbanItem,
  KanbanItemHandle,
  KanbanOverlay,
  type KanbanMoveEvent,
} from "@/components/reui/kanban";
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
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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
  exportSystemProfileJsonFile,
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
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";

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

const profileCatalogColumnValues: SourceStatus[] = [
  "draft",
  "active",
  "disabled",
  "invalid",
];

const profileCatalogColumnDescriptions: Record<SourceStatus, string> = {
  draft: "Noch in Vorbereitung.",
  active: "Für Quellen verwendbar.",
  disabled: "Bewusst pausiert.",
  invalid: "Benötigt Korrektur.",
};

type ProfileCatalogItem =
  | { id: string; type: "system"; profile: SystemProfile }
  | { id: string; type: "browser"; profile: BrowserProfile };

type ProfileCatalogColumns = Record<string, ProfileCatalogItem[]>;

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

type SourceDialogState = { mode: "create" } | { mode: "edit"; source: Source };

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
      const [browserProfiles, systemProfiles, sources, adapters] =
        await Promise.all([
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
  const [systemProfileExportDialogOpen, setSystemProfileExportDialogOpen] =
    useState(false);
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
    () =>
      new Map(
        browserProfiles.map((browserProfile) => [
          browserProfile.id,
          browserProfile,
        ]),
      ),
    [browserProfiles],
  );

  const systemProfilesById = useMemo(
    () =>
      new Map(
        systemProfiles.map((systemProfile) => [
          systemProfile.id,
          systemProfile,
        ]),
      ),
    [systemProfiles],
  );

  const adaptersByKey = useMemo(
    () => new Map(adapters.map((adapter) => [adapter.key, adapter])),
    [adapters],
  );

  const adapterOptions = useMemo(() => {
    const registeredOptions = sortAdaptersByUserFacingPriority(adapters).map(
      (adapter) => ({
        value: adapter.key,
        label: formatAdapterOptionLabel(adapter),
      }),
    );
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
          sourceMatchesFilter(
            source,
            filter,
            browserProfilesById,
            adaptersByKey,
          ),
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

  const handleCreateSystemProfile = async (input: CreateSystemProfileInput) => {
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

  const handleMoveBrowserProfileToStatus = async (
    browserProfile: BrowserProfile,
    status: SourceStatus,
  ) => {
    if (browserProfile.status === status) return;

    const updated = await updateBrowserProfile(
      browserProfile.id,
      browserProfileToUpdateInput(browserProfile, status),
    );
    toast.success("Browserprofil verschoben.", {
      description: `${updated.name} → ${sourceStatusLabels[updated.status]}`,
    });
    await refresh();
  };

  const handleMoveSystemProfileToStatus = async (
    systemProfile: SystemProfile,
    status: SourceStatus,
  ) => {
    if (systemProfile.status === status) return;

    const updated = await updateSystemProfile(
      systemProfile.id,
      systemProfileToUpdateInput(systemProfile, status),
    );
    toast.success("Systemprofil verschoben.", {
      description: `${updated.name} → ${sourceStatusLabels[updated.status]}`,
    });
    await refresh();
  };

  const handleExportSystemProfile = async (systemProfile: SystemProfile) => {
    try {
      const targetPath = await exportSystemProfileJsonFile(
        systemProfile.id,
        `${systemProfile.key}.json`,
      );

      if (!targetPath) {
        toast.info("Export abgebrochen.", { description: systemProfile.name });
        return true;
      }

      toast.success("Systemprofil exportiert.", {
        description: targetPath,
      });
      return true;
    } catch (unknownError) {
      toast.error("Systemprofil konnte nicht exportiert werden.", {
        description: String(unknownError),
      });
      return false;
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
    toast.success("Quelle gelöscht.", {
      description: deleteTarget.source.name,
    });
    if (selectedSourceId === deleteTarget.source.id) setSelectedSourceId(null);
    await refresh();
  };

  const deleteDialogCopy = getDeleteDialogCopy(deleteTarget);

  return (
    <div className="grid gap-5">
      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Quellen konnten nicht geladen werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <Tabs defaultValue="profiles" className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="space-y-1">
            <h1 className="text-3xl tracking-tight">Quellenverwaltung</h1>
            <p className="text-sm text-muted-foreground">
              Laufzeit, Profile und Quellen in getrennten Arbeitsbereichen.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => void refresh()}>
              <RefreshCwIcon className="size-4" aria-hidden="true" />
              Aktualisieren
            </Button>
            <TabsList className="gap-1">
              <TabsTrigger value="profiles">Profile</TabsTrigger>
              <TabsTrigger value="sources">Quellen</TabsTrigger>
              <TabsTrigger value="browserruntime">Browser-Laufzeit</TabsTrigger>
            </TabsList>
          </div>
        </div>

        <TabsContent value="profiles" className="grid gap-4">
          <SystemProfileTestRunner
            systemProfiles={systemProfiles}
            loading={loading}
          />
          <ProfileCatalog
            browserProfiles={browserProfiles}
            systemProfiles={systemProfiles}
            loading={loading}
            onCreateBrowserProfile={() =>
              setBrowserProfileDialog({ mode: "create" })
            }
            onEditBrowserProfile={(browserProfile) =>
              setBrowserProfileDialog({ mode: "edit", browserProfile })
            }
            onDeleteBrowserProfile={(browserProfile) =>
              setDeleteTarget({ type: "browserProfile", browserProfile })
            }
            onCreateSystemProfile={() =>
              setSystemProfileDialog({ mode: "create" })
            }
            onImportSystemProfile={() =>
              systemProfileImportInputRef.current?.click()
            }
            onOpenSystemProfileExport={() =>
              setSystemProfileExportDialogOpen(true)
            }
            onExportSystemProfile={(systemProfile) =>
              void handleExportSystemProfile(systemProfile)
            }
            onMoveBrowserProfileToStatus={handleMoveBrowserProfileToStatus}
            onMoveSystemProfileToStatus={handleMoveSystemProfileToStatus}
            onEditSystemProfile={(systemProfile) =>
              setSystemProfileDialog({ mode: "edit", systemProfile })
            }
            onDeleteSystemProfile={(systemProfile) =>
              setDeleteTarget({ type: "systemProfile", systemProfile })
            }
          />
        </TabsContent>

        <TabsContent value="sources" className="grid gap-4">
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

          <div className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(20rem,0.9fr)]">
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
                  ? (browserProfilesById.get(selectedSource.browserProfileId) ??
                    null)
                  : null
              }
              systemProfile={
                selectedSource?.systemProfileId
                  ? (systemProfilesById.get(selectedSource.systemProfileId) ??
                    null)
                  : null
              }
              loading={loading}
              adaptersByKey={adaptersByKey}
              onEdit={(source) => setSourceDialog({ mode: "edit", source })}
              onDelete={(source) => setDeleteTarget({ type: "source", source })}
            />
          </div>
        </TabsContent>

        <TabsContent value="browserruntime" className="grid gap-4">
          <BrowserRuntimeCard />
        </TabsContent>
      </Tabs>

      <input
        ref={systemProfileImportInputRef}
        type="file"
        accept="application/json,.json"
        className="hidden"
        onChange={(event) => void handleImportSystemProfileFile(event)}
      />

      <SystemProfileExportDialog
        open={systemProfileExportDialogOpen}
        systemProfiles={systemProfiles}
        onOpenChange={setSystemProfileExportDialogOpen}
        onExport={handleExportSystemProfile}
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
            handleUpdateSystemProfile(systemProfileDialog.systemProfile, input)
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
    <Card>
      <CardHeader>
        <CardTitle>Systemprofil testen</CardTitle>
        <CardDescription>
          Prüft eine URL gezielt gegen genau ein Systemprofil. Der Lauf zeigt
          Pflichtcheck-Evidence und den Quellenvorschlag, legt aber keine Quelle
          an und speichert nichts automatisch.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
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
          <p className="text-sm text-muted-foreground">
            Noch keine Systemprofile vorhanden.
          </p>
        ) : null}

        {error ? (
          <Alert variant="destructive">
            <AlertCircleIcon className="size-4" aria-hidden="true" />
            <AlertTitle>Profilcheck konnte nicht ausgeführt werden</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {result ? <SystemProfileTestResultView result={result} /> : null}
      </CardContent>
    </Card>
  );
}

function SystemProfileTestResultView({
  result,
}: {
  result: SystemProfileTestResult;
}) {
  const passed = result.status === "passed";

  return (
    <div className="grid gap-3">
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
          Keine Quellenkonfiguration wird vorgeschlagen, solange ein
          Pflichtcheck fehlschlägt.
        </p>
      ) : null}

      {passed && result.sourceConfig ? (
        <section className="grid gap-2">
          <h3 className="text-sm font-medium">
            Vorgeschlagene Quellenkonfiguration
          </h3>
          <div className="grid gap-2 rounded-md border p-3 text-sm">
            {result.name ? (
              <DetailRow label="Name" value={result.name} />
            ) : null}
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

function SystemProfileExportDialog({
  open,
  systemProfiles,
  onOpenChange,
  onExport,
}: {
  open: boolean;
  systemProfiles: SystemProfile[];
  onOpenChange: (open: boolean) => void;
  onExport: (systemProfile: SystemProfile) => Promise<boolean>;
}) {
  const [selectedSystemProfileId, setSelectedSystemProfileId] = useState("");
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    if (!open) return;
    setSelectedSystemProfileId(String(systemProfiles[0]?.id ?? ""));
    setExporting(false);
  }, [open, systemProfiles]);

  const selectedSystemProfile =
    systemProfiles.find(
      (systemProfile) => String(systemProfile.id) === selectedSystemProfileId,
    ) ?? null;

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedSystemProfile) return;

    setExporting(true);
    const completed = await onExport(selectedSystemProfile);
    setExporting(false);

    if (completed) onOpenChange(false);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!exporting) onOpenChange(nextOpen);
      }}
    >
      <DialogContent>
        <form
          className="grid gap-4"
          onSubmit={(event) => void handleSubmit(event)}
        >
          <DialogHeader>
            <DialogTitle>Systemprofil exportieren</DialogTitle>
            <DialogDescription>
              Wähle ein Systemprofil aus. Danach öffnet sich der native
              Speichern-Dialog deines Betriebssystems.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-1.5">
            <label
              className="text-xs font-medium"
              htmlFor="system-profile-export-select"
            >
              Systemprofil
            </label>
            <NativeSelect
              id="system-profile-export-select"
              className="w-full"
              value={selectedSystemProfileId}
              onChange={(event) =>
                setSelectedSystemProfileId(event.target.value)
              }
              disabled={exporting || systemProfiles.length === 0}
              required
            >
              {systemProfiles.map((systemProfile) => (
                <NativeSelectOption
                  key={systemProfile.id}
                  value={String(systemProfile.id)}
                >
                  {systemProfile.name} ({systemProfile.key})
                </NativeSelectOption>
              ))}
            </NativeSelect>
            <p className="text-xs text-muted-foreground">
              Der Speichern-Dialog startet im App-Datenordner für Systemprofile.
            </p>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={exporting}
            >
              Abbrechen
            </Button>
            <Button
              type="submit"
              disabled={exporting || !selectedSystemProfile}
            >
              <FileJsonIcon className="size-4" aria-hidden="true" />
              {exporting ? "Exportiere…" : "Exportieren…"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
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
  onOpenSystemProfileExport,
  onExportSystemProfile,
  onMoveBrowserProfileToStatus,
  onMoveSystemProfileToStatus,
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
  onOpenSystemProfileExport: () => void;
  onExportSystemProfile: (systemProfile: SystemProfile) => void;
  onMoveBrowserProfileToStatus: (
    browserProfile: BrowserProfile,
    status: SourceStatus,
  ) => Promise<void>;
  onMoveSystemProfileToStatus: (
    systemProfile: SystemProfile,
    status: SourceStatus,
  ) => Promise<void>;
  onEditSystemProfile: (systemProfile: SystemProfile) => void;
  onDeleteSystemProfile: (systemProfile: SystemProfile) => void;
}) {
  const derivedColumns = useMemo(
    () => buildProfileCatalogColumns(systemProfiles, browserProfiles),
    [browserProfiles, systemProfiles],
  );
  const [columns, setColumns] = useState<ProfileCatalogColumns>(derivedColumns);
  const profileCount = browserProfiles.length + systemProfiles.length;

  useEffect(() => {
    setColumns(derivedColumns);
  }, [derivedColumns]);

  const handleProfileMove = useCallback(
    async ({
      activeContainer,
      activeIndex,
      overContainer,
      overIndex,
    }: KanbanMoveEvent) => {
      if (!isSourceStatus(activeContainer) || !isSourceStatus(overContainer)) {
        return;
      }

      const activeItems = columns[activeContainer] ?? [];
      const movedItem = activeItems[activeIndex];
      if (!movedItem || isProfileCatalogItemLocked(movedItem)) return;

      if (activeContainer === overContainer) {
        if (activeIndex === overIndex) return;

        setColumns({
          ...columns,
          [activeContainer]: moveProfileCatalogItem(
            activeItems,
            activeIndex,
            overIndex,
          ),
        });
        return;
      }

      const previousColumns = columns;
      const nextActiveItems = [...activeItems];
      nextActiveItems.splice(activeIndex, 1);

      const nextOverItems = [...(columns[overContainer] ?? [])];
      const nextOverIndex = clampIndex(overIndex, nextOverItems.length);
      nextOverItems.splice(
        nextOverIndex,
        0,
        withProfileCatalogItemStatus(movedItem, overContainer),
      );

      setColumns({
        ...columns,
        [activeContainer]: nextActiveItems,
        [overContainer]: nextOverItems,
      });

      try {
        if (movedItem.type === "system") {
          await onMoveSystemProfileToStatus(movedItem.profile, overContainer);
          return;
        }

        await onMoveBrowserProfileToStatus(movedItem.profile, overContainer);
      } catch (unknownError) {
        setColumns(previousColumns);
        toast.error("Profilstatus konnte nicht geändert werden.", {
          description: String(unknownError),
        });
      }
    },
    [columns, onMoveBrowserProfileToStatus, onMoveSystemProfileToStatus],
  );

  return (
    <Frame className="content-start" spacing="sm">
      <FramePanel>
        <FrameHeader className="gap-3 px-0 pt-0 sm:flex-row sm:items-start sm:justify-between">
          <div className="grid gap-1">
            <FrameTitle>Katalog</FrameTitle>
            <FrameDescription>
              Systemprofile und Browserprofile als Kanban nach Quellstatus.
              Karten können zwischen Status-Spalten verschoben werden.
            </FrameDescription>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onOpenSystemProfileExport}
              disabled={loading || systemProfiles.length === 0}
            >
              <FileJsonIcon className="size-4" aria-hidden="true" />
              Export
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onImportSystemProfile}
            >
              <FileJsonIcon className="size-4" aria-hidden="true" />
              Import
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onCreateSystemProfile}
            >
              <PlusIcon className="size-4" aria-hidden="true" />
              System
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onCreateBrowserProfile}
            >
              <PlusIcon className="size-4" aria-hidden="true" />
              Browser
            </Button>
          </div>
        </FrameHeader>
        <ScrollArea>
          <div className="p-4">
            <Kanban
              value={columns}
              onValueChange={setColumns}
              getItemValue={(item) => item.id}
              onMove={(moveEvent) => void handleProfileMove(moveEvent)}
            >
              <KanbanBoard className="grid auto-rows-fr gap-3 sm:grid-cols-2 xl:grid-cols-4">
                {profileCatalogColumnValues.map((status) => (
                  <ProfileCatalogKanbanColumn
                    key={status}
                    value={status}
                    items={columns[status] ?? []}
                    loading={loading}
                    profileCount={profileCount}
                    onEditBrowserProfile={onEditBrowserProfile}
                    onDeleteBrowserProfile={onDeleteBrowserProfile}
                    onExportSystemProfile={onExportSystemProfile}
                    onEditSystemProfile={onEditSystemProfile}
                    onDeleteSystemProfile={onDeleteSystemProfile}
                  />
                ))}
              </KanbanBoard>
              <KanbanOverlay>
                {({ value, variant }) => {
                  if (variant === "column") {
                    const status = String(value);

                    return (
                      <ProfileCatalogKanbanColumn
                        value={status}
                        items={columns[status] ?? []}
                        loading={loading}
                        profileCount={profileCount}
                        isOverlay
                        onEditBrowserProfile={onEditBrowserProfile}
                        onDeleteBrowserProfile={onDeleteBrowserProfile}
                        onExportSystemProfile={onExportSystemProfile}
                        onEditSystemProfile={onEditSystemProfile}
                        onDeleteSystemProfile={onDeleteSystemProfile}
                      />
                    );
                  }

                  const item = Object.values(columns)
                    .flat()
                    .find((catalogItem) => catalogItem.id === value);

                  if (!item) return null;

                  return (
                    <ProfileCatalogCard
                      item={item}
                      isOverlay
                      onEditBrowserProfile={onEditBrowserProfile}
                      onDeleteBrowserProfile={onDeleteBrowserProfile}
                      onExportSystemProfile={onExportSystemProfile}
                      onEditSystemProfile={onEditSystemProfile}
                      onDeleteSystemProfile={onDeleteSystemProfile}
                    />
                  );
                }}
              </KanbanOverlay>
            </Kanban>
          </div>
          <ScrollBar orientation="horizontal" />
        </ScrollArea>
        {!loading && profileCount === 0 ? (
          <div className="mt-3 rounded-md border border-dashed p-4 text-center">
            <p className="text-sm font-medium">
              Noch keine Profile registriert.
            </p>
            <div className="mt-3 flex flex-wrap justify-center gap-2">
              <Button type="button" size="sm" onClick={onCreateSystemProfile}>
                <PlusIcon className="size-4" aria-hidden="true" />
                Systemprofil anlegen
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={onCreateBrowserProfile}
              >
                <PlusIcon className="size-4" aria-hidden="true" />
                Browserprofil anlegen
              </Button>
            </div>
          </div>
        ) : null}
      </FramePanel>
    </Frame>
  );
}

type ProfileCatalogCardActions = {
  onEditBrowserProfile: (browserProfile: BrowserProfile) => void;
  onDeleteBrowserProfile: (browserProfile: BrowserProfile) => void;
  onExportSystemProfile: (systemProfile: SystemProfile) => void;
  onEditSystemProfile: (systemProfile: SystemProfile) => void;
  onDeleteSystemProfile: (systemProfile: SystemProfile) => void;
};

function ProfileCatalogKanbanColumn({
  value,
  items,
  loading,
  profileCount,
  isOverlay = false,
  ...actions
}: ProfileCatalogCardActions & {
  value: string;
  items: ProfileCatalogItem[];
  loading: boolean;
  profileCount: number;
  isOverlay?: boolean;
}) {
  const status = isSourceStatus(value) ? value : "draft";

  return (
    <KanbanColumn value={value} className="min-w-0">
      <Card className="h-full bg-muted/20" size="sm">
        <CardHeader className="flex flex-row items-start justify-between gap-2">
          <div className="grid min-w-0 gap-1">
            <div className="flex items-center gap-2">
              <StatusBadge status={status} />
              <Badge variant="outline" size="xs">
                {items.length}
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground">
              {profileCatalogColumnDescriptions[status]}
            </p>
          </div>
        </CardHeader>
        <CardContent>
          <KanbanColumnContent value={value} className="min-h-32 gap-2 p-0.5">
            {loading ? (
              <MutedLine>Lade Profile…</MutedLine>
            ) : items.length ? (
              items.map((item) => (
                <ProfileCatalogCard
                  key={item.id}
                  item={item}
                  isOverlay={isOverlay}
                  {...actions}
                />
              ))
            ) : (
              <div className="rounded-md border border-dashed bg-background/60 p-3 text-center text-xs text-muted-foreground">
                {profileCount === 0
                  ? "Noch keine Profile."
                  : "Keine Profile in diesem Status."}
              </div>
            )}
          </KanbanColumnContent>
        </CardContent>
      </Card>
    </KanbanColumn>
  );
}

function ProfileCatalogCard({
  item,
  isOverlay = false,
  onEditBrowserProfile,
  onDeleteBrowserProfile,
  onExportSystemProfile,
  onEditSystemProfile,
  onDeleteSystemProfile,
}: ProfileCatalogCardActions & {
  item: ProfileCatalogItem;
  isOverlay?: boolean;
}) {
  const locked = isProfileCatalogItemLocked(item);
  const profile = item.profile;
  const typeLabel = item.type === "system" ? "System" : "Browser";
  const subtitle =
    item.type === "system"
      ? `${item.profile.adapterKey}`
      : `Schema v${item.profile.definitionSchemaVersion}`;

  const cardContent = (
    <Card
      className={cn(
        "bg-background transition-shadow",
        !locked && "hover:ring-foreground/20",
        isOverlay && "shadow-lg",
      )}
      size="sm"
    >
      <CardContent className="grid gap-1.5">
        <div
          className={cn(
            "grid min-w-0 gap-x-1.5 gap-y-0.5",
            !locked
              ? "grid-cols-[0.875rem_minmax(0,1fr)_auto]"
              : "grid-cols-[minmax(0,1fr)_auto]",
          )}
        >
          {!locked ? (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label={`Profil ${profile.name} verschieben`}
              title="Verschieben"
              className="col-start-1 row-start-1 size-3.5 translate-y-px self-baseline p-0 text-muted-foreground"
            >
              <GripVerticalIcon className="size-3" aria-hidden="true" />
            </Button>
          ) : null}

          <div
            className={cn(
              "row-start-1 self-baseline truncate text-sm font-medium leading-tight",
              !locked ? "col-start-2" : "col-start-1",
            )}
          >
            {profile.name}
          </div>

          <div
            className={cn(
              "row-start-1 flex shrink-0 items-baseline gap-1 self-baseline",
              !locked ? "col-start-3" : "col-start-2",
            )}
          >
            <Badge
              className="self-baseline"
              variant={item.type === "system" ? "primary-light" : "info-light"}
              size="xs"
            >
              {typeLabel}
            </Badge>
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    className="-my-1 self-center"
                    aria-label={`Aktionen für ${profile.name}`}
                    title="Aktionen"
                    onPointerDown={(event) => event.stopPropagation()}
                    onKeyDown={(event) => event.stopPropagation()}
                  >
                    <MoreHorizontalIcon aria-hidden="true" />
                  </Button>
                }
              />
              <DropdownMenuContent align="end" className="min-w-36">
                {item.type === "system" ? (
                  <DropdownMenuItem
                    onClick={() => onExportSystemProfile(item.profile)}
                  >
                    <FileJsonIcon aria-hidden="true" />
                    Exportieren
                  </DropdownMenuItem>
                ) : null}
                <DropdownMenuItem
                  onClick={() =>
                    item.type === "system"
                      ? onEditSystemProfile(item.profile)
                      : onEditBrowserProfile(item.profile)
                  }
                  disabled={locked}
                >
                  <PencilIcon aria-hidden="true" />
                  Bearbeiten
                </DropdownMenuItem>
                <DropdownMenuItem
                  variant="destructive"
                  onClick={() =>
                    item.type === "system"
                      ? onDeleteSystemProfile(item.profile)
                      : onDeleteBrowserProfile(item.profile)
                  }
                  disabled={locked}
                >
                  <Trash2Icon aria-hidden="true" />
                  Löschen
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>

          <p
            className={cn(
              "row-start-2 truncate text-[0.68rem] text-muted-foreground",
              !locked ? "col-start-2 col-end-3" : "col-start-1 col-end-2",
            )}
          >
            {subtitle}
          </p>
        </div>

        {profile.validationError ? (
          <p className="line-clamp-1 rounded-sm bg-destructive/10 px-1.5 py-0.5 text-[0.68rem] text-destructive">
            {profile.validationError}
          </p>
        ) : null}
      </CardContent>
    </Card>
  );

  return (
    <KanbanItem value={item.id} disabled={locked}>
      {!locked && !isOverlay ? (
        <KanbanItemHandle>{cardContent}</KanbanItemHandle>
      ) : (
        cardContent
      )}
    </KanbanItem>
  );
}

function buildProfileCatalogColumns(
  systemProfiles: SystemProfile[],
  browserProfiles: BrowserProfile[],
): ProfileCatalogColumns {
  const columns = Object.fromEntries(
    profileCatalogColumnValues.map((status) => [status, []]),
  ) as ProfileCatalogColumns;

  for (const systemProfile of systemProfiles) {
    columns[systemProfile.status].push({
      id: `system-${systemProfile.id}`,
      type: "system",
      profile: systemProfile,
    });
  }

  for (const browserProfile of browserProfiles) {
    columns[browserProfile.status].push({
      id: `browser-${browserProfile.id}`,
      type: "browser",
      profile: browserProfile,
    });
  }

  return columns;
}

function browserProfileToUpdateInput(
  browserProfile: BrowserProfile,
  status: SourceStatus,
): UpdateBrowserProfileInput {
  return {
    name: browserProfile.name,
    description: browserProfile.description,
    nameI18nKey: browserProfile.nameI18nKey,
    descriptionI18nKey: browserProfile.descriptionI18nKey,
    definitionPath: browserProfile.definitionPath,
    definitionHash: browserProfile.definitionHash,
    definitionSchemaVersion: browserProfile.definitionSchemaVersion,
    definition: browserProfile.definition,
    sourceConfigSchema: browserProfile.sourceConfigSchema,
    status,
    validationError: browserProfile.validationError,
  };
}

function systemProfileToUpdateInput(
  systemProfile: SystemProfile,
  status: SourceStatus,
): UpdateSystemProfileInput {
  return {
    name: systemProfile.name,
    description: systemProfile.description,
    adapterKey: systemProfile.adapterKey,
    definitionSchemaVersion: systemProfile.definitionSchemaVersion,
    definition: systemProfile.definition,
    sourceConfigSchema: systemProfile.sourceConfigSchema,
    status,
    validationError: systemProfile.validationError,
  };
}

function isSourceStatus(value: string): value is SourceStatus {
  return profileCatalogColumnValues.includes(value as SourceStatus);
}

function isProfileCatalogItemLocked(item: ProfileCatalogItem) {
  return item.type === "system" && item.profile.builtIn;
}

function withProfileCatalogItemStatus(
  item: ProfileCatalogItem,
  status: SourceStatus,
): ProfileCatalogItem {
  if (item.type === "system") {
    return {
      ...item,
      profile: { ...item.profile, status },
    };
  }

  return {
    ...item,
    profile: { ...item.profile, status },
  };
}

function moveProfileCatalogItem(
  items: ProfileCatalogItem[],
  fromIndex: number,
  toIndex: number,
) {
  const nextItems = [...items];
  const [movedItem] = nextItems.splice(fromIndex, 1);
  if (!movedItem) return nextItems;

  nextItems.splice(clampIndex(toIndex, nextItems.length), 0, movedItem);
  return nextItems;
}

function clampIndex(index: number, maxIndex: number) {
  return Math.min(Math.max(index, 0), maxIndex);
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
                title={
                  source.builtIn
                    ? "Eingebaute Job-Portale können nicht gelöscht werden."
                    : undefined
                }
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
              <DetailRow
                label="Adapter"
                value={adapterDisplay?.name ?? source.adapterKey}
              />
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
