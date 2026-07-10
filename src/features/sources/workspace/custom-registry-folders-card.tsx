import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { DatabaseInfo } from "@/lib/api/database";

type CustomRegistryFoldersCardProps = {
  data: DatabaseInfo | null;
  error: string | null;
  loading: boolean;
  onRefresh: () => Promise<unknown> | unknown;
};

export const customRegistryFoldersDescription =
  "Lege eigene JSON-Dateien in diesen App-Data-Ordnern ab. Die Source Registry lädt sie zusätzlich zu den integrierten Einträgen.";

export function customRegistryFolderEntries(data: DatabaseInfo) {
  return [
    {
      label: "Eigene Sources",
      pattern: "sources/*.json",
      path: data.sourcesDir,
    },
    {
      label: "Eigene Source Profiles",
      pattern: "source-profiles/*.json",
      path: data.sourceProfilesDir,
    },
  ];
}

export function CustomRegistryFoldersCard({
  data,
  error,
  loading,
  onRefresh,
}: CustomRegistryFoldersCardProps) {
  return (
    <Card size="sm">
      <CardHeader>
        <CardTitle>Eigene Registry-Dateien</CardTitle>
        <CardDescription>{customRegistryFoldersDescription}</CardDescription>
      </CardHeader>
      <CardContent>
        {error ? (
          <Alert variant="destructive">
            <AlertCircleIcon aria-hidden="true" />
            <AlertTitle>Registry-Ordner konnten nicht geladen werden</AlertTitle>
            <AlertDescription className="flex flex-wrap items-center gap-2">
              <span>{error}</span>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={loading}
                onClick={() => void onRefresh()}
              >
                <RefreshCwIcon data-icon="inline-start" aria-hidden="true" />
                Erneut versuchen
              </Button>
            </AlertDescription>
          </Alert>
        ) : loading || !data ? (
          <div className="grid gap-3" aria-label="Registry-Ordner werden geladen">
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-12 w-full" />
          </div>
        ) : (
          <dl className="grid gap-3 md:grid-cols-2">
            {customRegistryFolderEntries(data).map((folder) => (
              <RegistryFolder key={folder.pattern} {...folder} />
            ))}
          </dl>
        )}
      </CardContent>
    </Card>
  );
}

function RegistryFolder({
  label,
  pattern,
  path,
}: {
  label: string;
  pattern: string;
  path: string;
}) {
  return (
    <div className="grid min-w-0 gap-1 rounded-md border p-3">
      <dt className="font-medium">
        {label} · <code translate="no">{pattern}</code>
      </dt>
      <dd className="break-all text-muted-foreground">
        <code translate="no">{path}</code>
      </dd>
    </div>
  );
}
