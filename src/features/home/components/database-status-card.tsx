import { AlertCircleIcon, RefreshCw } from "lucide-react";

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useDatabaseInfo } from "@/hooks/use-database-info";
import { cn } from "@/lib/utils";

const databaseTimestampFormatter = new Intl.DateTimeFormat("de", {
  dateStyle: "medium",
  timeStyle: "short",
});

export function DatabaseStatusCard() {
  const { data, error, loading, refresh } = useDatabaseInfo();

  return (
    <section className="rounded-lg border bg-card p-4 text-card-foreground shadow-xs">
      <header className="mb-4 flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="grid gap-1.5">
          <h2 className="text-pretty text-sm font-semibold">SQLite Datenbank</h2>
          <p className="max-w-3xl text-sm text-muted-foreground">
            Die lokale Datenbank ist Runtime-/Cache-Schicht. Quellen und
            Quellenprofile kommen aus gebündelten Built-ins und lokalen
            JSON-Dateien im App-Data-Ordner.
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={loading}
          onClick={() => void refresh()}
        >
          <RefreshCw
            data-icon="inline-start"
            className={cn(loading && "motion-safe:animate-spin")}
            aria-hidden="true"
          />
          Aktualisieren
        </Button>
      </header>

      <div className="grid gap-4" aria-live="polite">
        <div className="flex items-center gap-2">
          <Badge variant={data ? "success" : "secondary"}>
            {loading ? "Prüfe…" : data ? "Verbunden" : "Nicht verbunden"}
          </Badge>
          {data ? (
            <span className="text-sm text-muted-foreground">
              SQLite <code translate="no">{data.sqliteVersion}</code>
            </span>
          ) : null}
        </div>

        {error ? (
          <Alert variant="destructive">
            <AlertCircleIcon aria-hidden="true" />
            <AlertTitle>Datenbankstatus konnte nicht geladen werden</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {data ? (
          <dl className="grid gap-3 text-sm md:grid-cols-2">
            <DatabaseInfoItem
              label="Initialisiert"
              value={formatDatabaseTimestamp(data.initializedAt)}
            />
            <DatabaseInfoItem label="App Data" value={data.appDataDir} code />
            <DatabaseInfoItem
              label="Datenbankdatei"
              value={data.databasePath}
              code
              wide
            />
            <DatabaseInfoItem
              label="Custom-Quellenprofile"
              value={data.sourceProfilesDir}
              code
            />
            <DatabaseInfoItem
              label="Custom-Quellen"
              value={data.sourcesDir}
              code
            />
          </dl>
        ) : null}
      </div>
    </section>
  );
}

function DatabaseInfoItem({
  label,
  value,
  code = false,
  wide = false,
}: {
  label: string;
  value: string;
  code?: boolean;
  wide?: boolean;
}) {
  return (
    <div className={cn("rounded-lg bg-muted p-3", wide && "md:col-span-2")}>
      <dt className="font-medium">{label}</dt>
      <dd className="mt-1 break-all text-muted-foreground">
        {code ? <code translate="no">{value}</code> : value}
      </dd>
    </div>
  );
}

function formatDatabaseTimestamp(value: string | null | undefined) {
  if (!value) return "gerade eben";

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;

  return databaseTimestampFormatter.format(date);
}
