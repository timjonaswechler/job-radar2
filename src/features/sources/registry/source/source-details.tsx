import { AccessPathDetails } from "@/features/sources/registry/access-path-details";
import { DetailRow } from "@/features/sources/registry/detail-row";
import { InlineDiagnostics } from "@/features/sources/registry/registry-diagnostics";
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
import type {
  RegistrySource,
  RegistrySourceProfile,
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
        title="sourceOverrides"
        description="Kontrollierte Source-spezifische Verhaltensänderungen für den ausgewählten Profilpfad."
        value={source.document.sourceOverrides}
        schemaRef={profileDslSchemaRefs.sourceOverrides}
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
