import { useCallback, useEffect, useMemo, useState } from "react";

import { AlertCircleIcon, RefreshCcwIcon } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { DeleteSearchRequestDialog } from "@/features/search-requests/components/delete-search-request-dialog";
import { SearchRequestFormDialog } from "@/features/search-requests/components/search-request-form-dialog";
import { SearchRequestsTable } from "@/features/search-requests/components/search-requests-table/table";
import { createSearchRequestRows, type SearchRequestTableRow } from "@/features/search-requests/model/search-request-row-model";
import {
  createSearchRequest,
  deleteSearchRequest,
  listSearchRequests,
  updateSearchRequest,
  type CreateSearchRequestInput,
  type SearchRequest,
  type UpdateSearchRequestInput,
} from "@/lib/api/search-requests";
import {
  getSourceProfileRegistrySnapshot,
  type RegistrySource,
} from "@/lib/api/sources";

type SearchRequestsData = {
  requests: SearchRequest[];
  sources: RegistrySource[];
};

export function SearchRequests() {
  const [data, setData] = useState<SearchRequestsData>({ requests: [], sources: [] });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [formRequest, setFormRequest] = useState<SearchRequest | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [deleteRow, setDeleteRow] = useState<SearchRequestTableRow | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [requests, registrySnapshot] = await Promise.all([
        listSearchRequests(),
        getSourceProfileRegistrySnapshot(),
      ]);
      setData({ requests, sources: registrySnapshot.sources });
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
          onCreate={() => {
            setFormRequest(null);
            setFormOpen(true);
          }}
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

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
