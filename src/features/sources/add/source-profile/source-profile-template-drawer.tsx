import { FileJsonIcon, XIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { documentDirectoryLabels } from "@/features/sources/labels";
import type { SourceRegistryDocumentKind } from "@/lib/api/sources";

import { profileTemplateSnippet } from "./source-profile-template";

type SourceProfileTemplateDrawerProps = {
  kind: Exclude<SourceRegistryDocumentKind, "source">;
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function SourceProfileTemplateDrawer({
  kind,
  open,
  onOpenChange,
}: SourceProfileTemplateDrawerProps) {
  const title = "Quellenprofil hinzufügen";
  const directory = documentDirectoryLabels[kind];

  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction="right">
      <DrawerContent
        className="h-full data-[vaul-drawer-direction=right]:w-[min(calc(100vw-115px),960px)]
  data-[vaul-drawer-direction=right]:sm:max-w-none"
      >
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
              {JSON.stringify(profileTemplateSnippet, null, 2)}
            </pre>
          </div>
        </div>
      </DrawerContent>
    </Drawer>
  );
}
