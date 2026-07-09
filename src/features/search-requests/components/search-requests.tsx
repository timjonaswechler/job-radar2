import { useCallback, useEffect, useMemo, useState } from "react";

import { AlertCircleIcon, RefreshCcwIcon } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { DeleteSearchRequestDialog } from "@/features/search-requests/components/delete-search-request-dialog";
import { SearchRequestFormDialog } from "@/features/search-requests/components/search-request-form-dialog";
import { SearchRunPanel } from "@/features/search-requests/components/search-run-panel";
import { SearchRequestsTable } from "@/features/search-requests/components/search-requests-table/table";
import { createSearchRequestRows, type SearchRequestTableRow } from "@/features/search-requests/model/search-request-row-model";
import {
  cancelBackgroundTask,
  createSearchRequest,
  deleteSearchRequest,
  getBackgroundTask,
  listSearchRequests,
  parseSearchRunResult,
  runSearchRequest,
  updateSearchRequest,
  type BackgroundTaskSnapshot,
  type CreateSearchRequestInput,
  type SearchRequest,
  type SearchRunResult,
  type UpdateSearchRequestInput,
} from "@/lib/api/search-requests";
import { getAppPreferences, type AppPreferences } from "@/lib/api/app-preferences";
import {
  getSourceProfileRegistrySnapshot,
  type RegistrySource,
} from "@/lib/api/sources";

type SearchRequestsData = {
  requests: SearchRequest[];
  sources: RegistrySource[];
  preferences: AppPreferences | null;
};

export function SearchRequests() {
  const [data, setData] = useState<SearchRequestsData>({
    requests: [],
    sources: [],
    preferences: null,
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [formRequest, setFormRequest] = useState<SearchRequest | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [deleteRow, setDeleteRow] = useState<SearchRequestTableRow | null>(null);
  const [activeRunRow, setActiveRunRow] = useState<SearchRequestTableRow | null>(null);
  const [runStarting, setRunStarting] = useState(false);
  const [runTask, setRunTask] = useState<BackgroundTaskSnapshot | null>(null);
  const [runResult, setRunResult] = useState<SearchRunResult | null>(null);
  const [runError, setRunError] = useState<string | null>(null);
  const [runCancelling, setRunCancelling] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [requests, registrySnapshot, preferences] = await Promise.all([
        listSearchRequests(),
        getSourceProfileRegistrySnapshot(),
        getAppPreferences(),
      ]);
      setData({ requests, sources: registrySnapshot.sources, preferences });
    } catch (unknownError) {
      const message = errorMessage(unknownError);
      setError(message);
      toast.error("Search Requests konnten nicht geladen werden", {
        description: message,
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const rows = useMemo(
    () => createSearchRequestRows(data.requests, data.sources),
    [data.requests, data.sources],
  );
  const validationErrorCount = rows.filter((row) => row.validationError).length;
  const missingSourceKeyCount = rows.reduce(
    (count, row) => count + row.missingSourceKeys.length,
    0,
  );

  const handleTerminalSnapshot = useCallback(
    async (snapshot: BackgroundTaskSnapshot) => {
      const result = parseSearchRunResult(snapshot.result);
      setRunCancelling(false);
      setRunResult(result);

      if (snapshot.state === "failed") {
        const message = snapshot.error ?? "Search Run fehlgeschlagen.";
        setRunError(message);
        toast.error("Search Run fehlgeschlagen", { description: message });
      } else if (snapshot.state === "cancelled") {
        setRunError(null);
        toast.info("Search Run abgebrochen", {
          description: snapshot.error ?? undefined,
        });
      } else if (!result) {
        const message = "Das Search Run-Ergebnis konnte nicht gelesen werden.";
        setRunError(message);
        toast.error("Search Run-Ergebnis fehlt", { description: message });
      } else if (result.status === "completed") {
        setRunError(null);
        toast.success("Search Run abgeschlossen");
      } else if (result.status === "completed_with_errors") {
        setRunError(null);
        toast.warning("Search Run mit Source-Fehlern abgeschlossen");
      } else if (result.status === "cancelled") {
        setRunError(null);
        toast.info("Search Run abgebrochen");
      } else {
        setRunError(null);
        toast.error("Search Run fehlgeschlagen");
      }

      await refresh();
    },
    [refresh],
  );

  useEffect(() => {
    if (!runTask || !isInFlightBackgroundTask(runTask)) return;

    const taskId = runTask.taskId;
    let ignore = false;
    const timeoutId = window.setTimeout(() => {
      void (async () => {
        try {
          const nextSnapshot = await getBackgroundTask(taskId);
          if (ignore) return;
          setRunTask(nextSnapshot);
          if (isTerminalBackgroundTask(nextSnapshot)) {
            await handleTerminalSnapshot(nextSnapshot);
          }
        } catch (unknownError) {
          if (ignore) return;
          const message = errorMessage(unknownError);
          setRunTask(null);
          setRunCancelling(false);
          setRunError(message);
          toast.error("Search Run-Status konnte nicht geladen werden", {
            description: message,
          });
        }
      })();
    }, 1000);

    return () => {
      ignore = true;
      window.clearTimeout(timeoutId);
    };
  }, [handleTerminalSnapshot, runTask]);

  const runningRequestId =
    activeRunRow && (runStarting || isInFlightBackgroundTask(runTask))
      ? activeRunRow.id
      : null;

  const handleSubmit = async (
    input: CreateSearchRequestInput | UpdateSearchRequestInput,
    request: SearchRequest | null,
  ) => {
    if (request) {
      await updateSearchRequest(request.id, input);
      toast.success("Search Request aktualisiert");
    } else {
      await createSearchRequest(input);
      toast.success("Search Request erstellt");
    }
    await refresh();
  };

  const handleDelete = async () => {
    if (!deleteRow) return;
    await deleteSearchRequest(deleteRow.id);
    toast.success("Search Request gelöscht");
    setDeleteRow(null);
    await refresh();
  };

  const handleRun = useCallback(
    async (row: SearchRequestTableRow) => {
      setActiveRunRow(row);
      setRunStarting(true);
      setRunTask(null);
      setRunResult(null);
      setRunError(null);
      setRunCancelling(false);

      try {
        const snapshot = await runSearchRequest(row.id);
        setRunTask(snapshot);
        toast.info("Search Run gestartet", { description: row.title });

        if (isTerminalBackgroundTask(snapshot)) {
          await handleTerminalSnapshot(snapshot);
        }
      } catch (unknownError) {
        const message = errorMessage(unknownError);
        setRunTask(null);
        setRunCancelling(false);
        setRunError(message);
        toast.error("Search Run konnte nicht gestartet werden", {
          description: message,
        });
      } finally {
        setRunStarting(false);
      }
    },
    [handleTerminalSnapshot],
  );

  const handleCancelRun = useCallback(async () => {
    if (!runTask || (runTask.state !== "queued" && runTask.state !== "running")) return;

    setRunCancelling(true);
    try {
      const snapshot = await cancelBackgroundTask(runTask.taskId);
      setRunTask(snapshot);
      if (isTerminalBackgroundTask(snapshot)) {
        await handleTerminalSnapshot(snapshot);
      } else {
        toast.info("Search Run wird abgebrochen");
      }
    } catch (unknownError) {
      const message = errorMessage(unknownError);
      setRunError(message);
      toast.error("Search Run konnte nicht abgebrochen werden", {
        description: message,
      });
    } finally {
      setRunCancelling(false);
    }
  }, [handleTerminalSnapshot, runTask]);

  return (
    <div className="grid gap-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="grid gap-1">
          <h1 className="text-xl font-semibold tracking-tight">Search Requests</h1>
          <p className="max-w-3xl text-sm text-muted-foreground">
            Verwalte wiederholbare Jobsuchen mit Titel-Regeln, Orten, Radius und Source Keys aus der aktuellen Source Registry.
          </p>
        </div>
        <Button type="button" variant="outline" onClick={() => void refresh()} disabled={loading}>
          <RefreshCcwIcon data-icon="inline-start" aria-hidden="true" />
          Aktualisieren
        </Button>
      </div>

      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Search Requests konnten nicht geladen werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {activeRunRow || runTask || runResult || runError ? (
        <SearchRunPanel
          row={activeRunRow}
          starting={runStarting}
          task={runTask}
          result={runResult}
          error={runError}
          cancelling={runCancelling}
          onCancel={handleCancelRun}
        />
      ) : null}

      {validationErrorCount || missingSourceKeyCount ? (
        <Alert variant="warning">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Einige Search Requests brauchen Aufmerksamkeit</AlertTitle>
          <AlertDescription>
            {validationErrorCount ? `${validationErrorCount} mit Backend-Validierungsfehler. ` : null}
            {missingSourceKeyCount ? `${missingSourceKeyCount} ausgewählte Source Keys fehlen in der aktuellen Registry.` : null}
          </AlertDescription>
        </Alert>
      ) : null}

      {loading ? (
        <SearchRequestsSkeleton />
      ) : (
        <SearchRequestsTable
          rows={rows}
          runningRequestId={runningRequestId}
          onCreate={() => {
            setFormRequest(null);
            setFormOpen(true);
          }}
          onRun={handleRun}
          onEdit={(row) => {
            setFormRequest(row.request);
            setFormOpen(true);
          }}
          onDelete={setDeleteRow}
        />
      )}

      <SearchRequestFormDialog
        open={formOpen}
        request={formRequest}
        sources={data.sources}
        defaultSearchRadiusKm={data.preferences?.defaultSearchRadiusKm ?? null}
        onOpenChange={setFormOpen}
        onSubmit={handleSubmit}
      />

      <DeleteSearchRequestDialog
        row={deleteRow}
        open={deleteRow !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteRow(null);
        }}
        onConfirm={handleDelete}
      />
    </div>
  );
}

function SearchRequestsSkeleton() {
  return (
    <Card>
      <CardHeader>
        <Skeleton className="h-6 w-48" />
        <Skeleton className="h-4 w-96" />
      </CardHeader>
      <CardContent className="grid gap-3">
        {Array.from({ length: 5 }).map((_, index) => (
          <Skeleton key={index} className="h-12 w-full" />
        ))}
      </CardContent>
    </Card>
  );
}

function isInFlightBackgroundTask(task: BackgroundTaskSnapshot | null) {
  return task?.state === "queued" || task?.state === "running" || task?.state === "cancelling";
}

function isTerminalBackgroundTask(task: BackgroundTaskSnapshot) {
  return task.state === "succeeded" || task.state === "failed" || task.state === "cancelled";
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
