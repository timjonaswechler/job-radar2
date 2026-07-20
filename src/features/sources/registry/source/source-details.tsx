import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { DetailRow } from "@/features/sources/registry/detail-row";
import { InlineDiagnostics } from "@/features/sources/registry/diagnostics/inline-diagnostics";
import { SourceLiveCheckSection } from "@/features/sources/registry/source/source-live-check-section";
import { OptionalJsonPreview } from "@/features/sources/shared/json-preview";
import { profileDslSchemaRefs } from "@/features/sources/shared/profile-dsl-schema-catalog";
import {
  OptionalSchemaValuePreview,
  SchemaValuePreview,
} from "@/features/sources/shared/schema-value-table";
import {
  originLabels,
  supportLevelLabels,
  validationStateLabels,
} from "@/features/sources/labels";
import { sourceStatusLabels } from "@/features/sources/status";
import { resolveSource } from "@/features/sources/view-model/registry-resolution";
import type { SourceResolution } from "@/features/sources/view-model/registry-resolution";
import type {
  ProfileAccessPathDefinition,
  RegistrySource,
  RegistrySourceProfile,
  SelectedAccessPath,
  SourceOwnedSelectedAccessPath,
  StructuredDiagnostic,
} from "@/lib/api/sources";

type SourceDetailsProps = {
  source: RegistrySource;
  profilesByKey: Map<string, RegistrySourceProfile>;
  diagnostics: StructuredDiagnostic[];
  onUpdated?: () => Promise<unknown> | unknown;
};

export function SourceDetails({
  source,
  profilesByKey,
  diagnostics,
  onUpdated,
}: SourceDetailsProps) {
  const selectedAccessPath = source.document.selectedAccessPath;
  const resolution = resolveSource(source, profilesByKey);
  const validationDiagnostics = source.validationState.diagnostics ?? [];

  return (
    <div className="grid gap-4 py-4 text-sm">
      {diagnostics.length ? (
        <InlineDiagnostics
          title="Diagnosen zu dieser Source"
          diagnostics={diagnostics}
        />
      ) : null}
      {validationDiagnostics.length ? (
        <InlineDiagnostics
          title="Validation-State-Diagnosen"
          diagnostics={validationDiagnostics}
        />
      ) : null}

      <SourceLiveCheckSection source={source} onUpdated={onUpdated} />

      <dl className="grid gap-3 sm:grid-cols-2">
        <DetailRow label="Source Key" value={source.document.key} mono />
        <DetailRow label="Name" value={source.document.name} />
        <DetailRow
          label="Source Status"
          value={sourceStatusLabels[source.document.status]}
        />
        <DetailRow
          label="Validation State"
          value={validationStateLabels[source.validationState.state]}
        />
        <DetailRow
          label="Kann kompilieren"
          value={source.validationState.canCompile ? "Ja" : "Nein"}
        />
        <DetailRow
          label="Kann ausführen"
          value={source.validationState.canExecute ? "Ja" : "Nein"}
        />
        <DetailRow
          label="Deklarierter Profil-/Access-Path-Support"
          value={
            resolution.supportLevel
              ? supportLevelLabels[resolution.supportLevel]
              : "—"
          }
        />
        <DetailRow label="Ursprung" value={originLabels[source.origin]} />
        <DetailRow label="Registry-Dokument" value={source.path} mono />
      </dl>

      <SchemaValuePreview
        title="sourceConfig"
        description="Stabile Zugriffskonfiguration der Source. Search Request Kriterien gehören nicht hierher."
        value={source.document.sourceConfig}
        schema={resolution.effectiveSourceConfigSchema}
      />
      <SchemaValuePreview
        title="Effektives sourceConfigSchema"
        description="Profil- und Access-Path-Schema, wie die Registry sie für diese Source zusammenführt."
        value={resolution.effectiveSourceConfigSchema}
      />

      <AccessPathDetails
        selectedAccessPath={selectedAccessPath}
        resolution={resolution}
      />

      <OptionalSchemaValuePreview
        title="Authored accessPaths"
        description="Direkt im schema-v3 Source-Dokument authorisierte Spezialisierungsfragmente."
        value={source.document.accessPaths}
        schemaRef={profileDslSchemaRefs.accessPathFragments}
      />
      <OptionalSchemaValuePreview
        title="sourceSupport"
        description="Support-Metadaten für Source-owned Access Paths."
        value={source.document.sourceSupport}
        schemaRef={profileDslSchemaRefs.supportMetadata}
      />
      <OptionalJsonPreview
        title="Source-Diagnosen im Dokument"
        description="Im Source-Dokument gespeicherte strukturierte Diagnosen."
        value={source.document.diagnostics}
      />
    </div>
  );
}

type AccessPathDetailsProps = {
  selectedAccessPath: SelectedAccessPath;
  resolution: SourceResolution;
};

function AccessPathDetails({
  selectedAccessPath,
  resolution,
}: AccessPathDetailsProps) {
  if (selectedAccessPath.type === "profile_access_path") {
    return (
      <div className="grid gap-3 rounded-lg border p-3 text-sm">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <p className="font-medium">Effektiver Profil-Zugriffspfad</p>
            <p className="text-xs text-muted-foreground">
              Vom Backend-Compiler aus Basisprofil und authored accessPaths
              materialisiertes Verhalten.
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
          {resolution.supportLevel ? (
            <DetailRow
              label="Support"
              value={supportLevelLabels[resolution.supportLevel]}
            />
          ) : null}
          {resolution.profileAccessPath ? (
            <DetailRow
              label="Pfad-Name"
              value={resolution.profileAccessPath.name}
            />
          ) : null}
          <DetailRow
            label="Fähigkeiten"
            value={resolution.capabilities.join(", ") || "—"}
          />
        </dl>
        {resolution.profileAccessPath ? (
          <AccessPathJsonBlocks accessPath={resolution.profileAccessPath} />
        ) : (
          <Alert variant="warning">
            <AlertCircleIcon className="size-4" aria-hidden="true" />
            <AlertTitle>Zugriffspfad nicht gefunden</AlertTitle>
            <AlertDescription>
              Die Registry sollte diese Source nicht als ausführbar markieren,
              wenn das Profil oder der Pfad fehlt. Bitte Diagnosen prüfen.
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
          <p className="font-medium">Source-owned Access Path</p>
          <p className="text-xs text-muted-foreground">
            Der Access Path ist direkt im Source-Dokument eingebettet und gehört
            nicht zu einem wiederverwendbaren Source Profile.
          </p>
        </div>
        <Badge variant="secondary">source-owned</Badge>
      </div>
      <dl className="grid gap-3 sm:grid-cols-2">
        <DetailRow label="Pfad-Key" value={selectedAccessPath.key} mono />
        <DetailRow label="Pfad-Name" value={selectedAccessPath.name} />
        {resolution.supportLevel ? (
          <DetailRow
            label="Support"
            value={supportLevelLabels[resolution.supportLevel]}
          />
        ) : null}
        <DetailRow
          label="Fähigkeiten"
          value={resolution.capabilities.join(", ") || "—"}
        />
      </dl>
      <AccessPathJsonBlocks accessPath={selectedAccessPath} />
    </div>
  );
}

type AccessPathJsonBlocksProps = {
  accessPath: ProfileAccessPathDefinition | SourceOwnedSelectedAccessPath;
};

function AccessPathJsonBlocks({ accessPath }: AccessPathJsonBlocksProps) {
  return (
    <div className="grid gap-2">
      {accessPath.diagnostics?.length ? (
        <InlineDiagnostics
          title="Diagnosen zu diesem Access Path"
          diagnostics={accessPath.diagnostics}
        />
      ) : null}
      <OptionalSchemaValuePreview
        title="sourceConfigSchema"
        description="Path-spezifisches Schema für Source Config. Search Request Kriterien gehören nicht hierher."
        value={accessPath.sourceConfigSchema}
      />
      {"knownIssues" in accessPath ? (
        <OptionalSchemaValuePreview
          title="knownIssues"
          description="Bekannte Einschränkungen dieses Access Path."
          value={accessPath.knownIssues}
        />
      ) : null}
      <OptionalSchemaValuePreview
        title="discovery"
        description="Deklarative source-weite Posting Discovery."
        value={accessPath.discovery}
        schemaRef={profileDslSchemaRefs.discoveryStep}
      />
      <OptionalSchemaValuePreview
        title="detail"
        description="Optionale lazy Posting Detail Extraktion für eine konkrete Posting-Quelle."
        value={accessPath.detail}
        schemaRef={profileDslSchemaRefs.detailStep}
      />
    </div>
  );
}
