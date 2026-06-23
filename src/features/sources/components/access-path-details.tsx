import { useState } from "react";

import { AlertCircleIcon, ChevronDownIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  formatAdapterCategory,
  formatAdapterRisk,
  getAdapterDisplay,
} from "@/features/sources/adapter-metadata";
import { DetailRow } from "@/features/sources/components/detail-row";
import { OptionalJsonPreview } from "@/features/sources/components/json-preview";
import { adapterExecutionModeLabels } from "@/features/sources/labels";
import {
  formatBoolean,
  type SourceResolution,
} from "@/features/sources/registry-view-model";
import type {
  AdapterMetadata,
  ProfileAccessPathDefinition,
  SelectedAccessPath,
  SourceSpecificSelectedAccessPath,
} from "@/lib/api/sources";

type AccessPathDetailsProps = {
  selectedAccessPath: SelectedAccessPath;
  resolution: SourceResolution;
};

export function AccessPathDetails({
  selectedAccessPath,
  resolution,
}: AccessPathDetailsProps) {
  if (selectedAccessPath.type === "profile") {
    return (
      <div className="grid gap-3 rounded-lg border p-3 text-sm">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <p className="font-medium">Profil-Zugriffspfad</p>
            <p className="text-xs text-muted-foreground">
              Quelle referenziert ein wiederverwendbares Quellenprofil und einen
              dort definierten Zugriffspfad.
            </p>
          </div>
          <Badge
            variant={
              resolution.profileAccessPath ? "success-light" : "warning-light"
            }
          >
            {resolution.profileAccessPath ? "aufgelöst" : "nicht aufgelöst"}
          </Badge>
        </div>
        <dl className="grid gap-3 sm:grid-cols-2">
          <DetailRow
            label="Profil-Key"
            value={selectedAccessPath.profileKey}
            mono
          />
          <DetailRow label="Pfad-Key" value={selectedAccessPath.pathKey} mono />
          {resolution.profile ? (
            <DetailRow
              label="Profil-Name"
              value={resolution.profile.document.name}
            />
          ) : null}
          {resolution.profileAccessPath ? (
            <DetailRow
              label="Pfad-Name"
              value={
                resolution.profileAccessPath.name ??
                resolution.profileAccessPath.key
              }
            />
          ) : null}
        </dl>
        {resolution.profileAccessPath ? (
          <AccessPathJsonBlocks accessPath={resolution.profileAccessPath} />
        ) : (
          <Alert variant="warning">
            <AlertCircleIcon className="size-4" aria-hidden="true" />
            <AlertTitle>Zugriffspfad nicht gefunden</AlertTitle>
            <AlertDescription>
              Die Registry sollte diese Quelle nicht als gültig laden, wenn das
              Profil oder der Pfad fehlt. Bitte Diagnosen prüfen.
            </AlertDescription>
          </Alert>
        )}
      </div>
    );
  }

  return (
    <div className="grid gap-3 rounded-lg border p-3 text-sm">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <p className="font-medium">Quellenspezifischer Zugriffspfad</p>
          <p className="text-xs text-muted-foreground">
            Der technische Zugriffspfad ist direkt im Source-Dokument
            eingebettet und gehört nicht zu einem wiederverwendbaren Profil.
          </p>
        </div>
        <Badge variant="secondary">source_specific</Badge>
      </div>
      <dl className="grid gap-3 sm:grid-cols-2">
        <DetailRow
          label="Adapter-Key"
          value={selectedAccessPath.adapterKey}
          mono
        />
      </dl>
      <AccessPathJsonBlocks accessPath={selectedAccessPath} />
    </div>
  );
}

type ProfileAccessPathRowProps = {
  accessPath: ProfileAccessPathDefinition;
  adapter: AdapterMetadata | undefined;
};

export function ProfileAccessPathRow({
  accessPath,
  adapter,
}: ProfileAccessPathRowProps) {
  const [open, setOpen] = useState(false);
  const adapterDisplay = getAdapterDisplay(accessPath.adapterKey, adapter);

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border p-3"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="font-medium">{accessPath.name ?? accessPath.key}</p>
          <p className="break-all font-mono text-xs text-muted-foreground">
            {accessPath.key} · {adapterDisplay.key}
          </p>
        </div>
        <div className="flex flex-wrap justify-end gap-1">
          <Badge
            variant={adapterDisplay.registered ? "secondary" : "warning-light"}
          >
            {adapterDisplay.registered ? "registriert" : "unregistriert"}
          </Badge>
          {adapter ? (
            <Badge variant="outline">{formatAdapterCategory(adapter)}</Badge>
          ) : null}
          <CollapsibleTrigger
            render={
              <Button
                type="button"
                variant="ghost"
                size="xs"
                className="group"
              />
            }
          >
            <ChevronDownIcon
              data-icon="inline-start"
              className="transition-transform group-data-[state=open]:rotate-180"
              aria-hidden="true"
            />
            Details
          </CollapsibleTrigger>
        </div>
      </div>
      <CollapsibleContent className="mt-3 grid gap-3">
        <dl className="grid gap-3 sm:grid-cols-2">
          <DetailRow label="Pfad-Key" value={accessPath.key} mono />
          <DetailRow label="Adapter-Key" value={accessPath.adapterKey} mono />
          {adapter ? (
            <>
              <DetailRow label="Adapter" value={adapter.name} />
              <DetailRow
                label="Ausführung"
                value={adapterExecutionModeLabels[adapter.executionMode]}
              />
              <DetailRow label="Risiko" value={formatAdapterRisk(adapter)} />
              <DetailRow
                label="Manuelle Freigabe"
                value={formatBoolean(adapter.supportsManualRelease)}
              />
            </>
          ) : null}
        </dl>
        {adapter?.description ? (
          <p className="text-xs text-muted-foreground">{adapter.description}</p>
        ) : null}
        <AccessPathJsonBlocks accessPath={accessPath} />
      </CollapsibleContent>
    </Collapsible>
  );
}

type AccessPathJsonBlocksProps = {
  accessPath: ProfileAccessPathDefinition | SourceSpecificSelectedAccessPath;
};

function AccessPathJsonBlocks({ accessPath }: AccessPathJsonBlocksProps) {
  return (
    <div className="grid gap-2">
      {"availability" in accessPath ? (
        <OptionalJsonPreview
          title="availability"
          description="Erforderliche Captures, Checks und vorgeschlagene Source-Konfiguration für diesen Profilpfad."
          value={accessPath.availability}
        />
      ) : null}
      <OptionalJsonPreview
        title="sourceConfigSchema"
        description="Path-spezifisches Schema für sourceConfig."
        value={accessPath.sourceConfigSchema}
      />
      <OptionalJsonPreview
        title="query"
        description="Suchparameterisierte Anfrage-Definition für diesen Zugriffspfad."
        value={accessPath.query}
      />
      <OptionalJsonPreview
        title="inventory"
        description="Deklarative Extraktion des Quellenbestands."
        value={accessPath.inventory}
      />
      <OptionalJsonPreview
        title="interactions"
        description="Begrenzte Browser-Interaktionen vor der Extraktion."
        value={accessPath.interactions}
      />
      <OptionalJsonPreview
        title="manualRelease"
        description="Metadaten für manuelle Freigabe, falls der Adapter sie unterstützt."
        value={accessPath.manualRelease}
      />
    </div>
  );
}
