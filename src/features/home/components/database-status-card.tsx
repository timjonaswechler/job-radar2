import { RefreshCw } from "lucide-react";

import { Frame } from "@/components/reui/frame";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useDatabaseInfo } from "@/hooks/use-database-info";

export function DatabaseStatusCard() {
  const { data, error, loading, refresh } = useDatabaseInfo();

  return (
    <Frame
      title="SQLite Datenbank"
      description="Die lokale Datenbank bleibt als Infrastruktur erhalten. Domain-Tabellen bauen wir später bewusst Schritt für Schritt auf."
      action={
        <Button variant="outline" size="sm" onClick={() => void refresh()}>
          <RefreshCw className="size-4" aria-hidden="true" />
          Aktualisieren
        </Button>
      }
    >
      <div className="grid gap-4">
        <div className="flex items-center gap-2">
          <Badge variant={data ? "success" : "secondary"}>
            {loading ? "Prüfe…" : data ? "Verbunden" : "Nicht verbunden"}
          </Badge>
          {data ? (
            <span className="text-sm text-muted-foreground">
              SQLite {data.sqliteVersion}
            </span>
          ) : null}
        </div>

        {error ? (
          <p className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
            {error}
          </p>
        ) : null}

        {data ? (
          <dl className="grid gap-3 text-sm md:grid-cols-2">
            <div className="rounded-lg bg-muted p-3">
              <dt className="font-medium">Initialisiert</dt>
              <dd className="mt-1 text-muted-foreground">
                {data.initializedAt ?? "gerade eben"}
              </dd>
            </div>
            <div className="rounded-lg bg-muted p-3">
              <dt className="font-medium">App Data</dt>
              <dd className="mt-1 break-all text-muted-foreground">{data.appDataDir}</dd>
            </div>
            <div className="rounded-lg bg-muted p-3 md:col-span-2">
              <dt className="font-medium">Datenbankdatei</dt>
              <dd className="mt-1 break-all text-muted-foreground">
                {data.databasePath}
              </dd>
            </div>
          </dl>
        ) : null}
      </div>
    </Frame>
  );
}
