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
import { SourceAddDrawer } from "@/features/sources/add/source-add-drawer";
import { documentDirectoryLabels } from "@/features/sources/labels";
import type {
  JsonValue,
  RegistrySource,
  RegistrySourceProfile,
  SourceRegistryDocumentKind,
} from "@/lib/api/sources";

type AddRegistryDocumentDrawerProps = {
  kind: SourceRegistryDocumentKind | null;
  open: boolean;
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  onCreated?: () => Promise<unknown> | unknown;
  onOpenChange: (open: boolean) => void;
};

export function AddRegistryDocumentDrawer({
  kind,
  open,
  profiles,
  sources,
  onCreated,
  onOpenChange,
}: AddRegistryDocumentDrawerProps) {
  if (!kind) {
    return <Drawer open={open} onOpenChange={onOpenChange} direction="right" />;
  }

  if (kind === "source") {
    return (
      <SourceAddDrawer
        open={open}
        profiles={profiles}
        sources={sources}
        onCreated={onCreated}
        onOpenChange={onOpenChange}
      />
    );
  }

  const title = "Quellenprofil hinzufügen";
  const directory = documentDirectoryLabels[kind];

  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction="right">
      <DrawerContent className="h-full sm:max-w-xl lg:max-w-2xl">
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

const profileTemplateSnippet: JsonValue = {
  schemaVersion: 2,
  key: "example_profile",
  name: "Example Profile",
  kind: "generic",
  support: {
    level: "experimental",
    summary: "Startpunkt für ein neues deklaratives Source Profile.",
  },
  sourceConfigSchema: {
    type: "object",
    required: ["startUrl"],
    additionalProperties: false,
    properties: {
      startUrl: {
        type: "string",
        format: "uri",
        title: "Start URL",
      },
    },
  },
  accessPaths: [
    {
      key: "html_jobs",
      name: "HTML jobs page",
      postingDiscovery: {
        strategies: [
          {
            key: "jobs_html",
            fetch: {
              mode: "http",
              method: "GET",
              url: "{{sourceConfig:startUrl}}",
              timeoutMs: 10000,
            },
            parse: { type: "html" },
            select: { type: "css", selector: ".job" },
            extract: {
              fields: {
                title: {
                  type: "css_text",
                  selector: ".title",
                  cardinality: "one",
                },
                company: {
                  type: "template",
                  template: "{{source:name}}",
                  cardinality: "one",
                },
                url: {
                  type: "css_attribute",
                  selector: "a",
                  attribute: "href",
                  cardinality: "one",
                },
              },
            },
          },
        ],
      },
    },
  ],
};
