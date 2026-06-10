import { RefreshCw } from "lucide-react";

import {
  Frame,
  FrameDescription,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useDatabaseInfo } from "@/hooks/use-database-info";

export function DatabaseStatusCard() {
  const { data, error, loading, refresh } = useDatabaseInfo();

  return (
    <Frame>
      <FramePanel>
        <FrameHeader className="gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="grid gap-1.5">
            <FrameTitle>SQLite Datenbank</FrameTitle>
            <FrameDescription>
              Die lokale Datenbank ist Runtime-/Cache-Schicht. Systemprofile kommen
              aus gebündelten Built-ins und lokalen JSON-Dateien im App-Data-Ordner.
            </FrameDescription>
          </div>
          <Button variant="outline" size="sm" onClick={() => void refresh()}>
            <RefreshCw className="size-4" aria-hidden="true" />
            Aktualisieren
          </Button>
        </FrameHeader>

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
                <dd className="mt-1 break-all text-muted-foreground">
                  {data.appDataDir}
                </dd>
              </div>
              <div className="rounded-lg bg-muted p-3 md:col-span-2">
                <dt className="font-medium">Datenbankdatei</dt>
                <dd className="mt-1 break-all text-muted-foreground">
                  {data.databasePath}
                </dd>
              </div>
              <div className="rounded-lg bg-muted p-3 md:col-span-2">
                <dt className="font-medium">Custom-Systemprofile</dt>
                <dd className="mt-1 break-all text-muted-foreground">
                  {data.systemProfilesDir}
                </dd>
              </div>
            </dl>
          ) : null}
        </div>
      </FramePanel>
    </Frame>
  );
}
