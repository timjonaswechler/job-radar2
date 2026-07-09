import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { DetailRow } from "@/features/sources/registry/detail-row";
import { OptionalSchemaValuePreview } from "@/features/sources/shared/schema-value-table";
import { profileDslSchemaRefs } from "@/features/sources/shared/profile-dsl-schema-catalog";
import { InlineDiagnostics } from "@/features/sources/registry/registry-diagnostics";
import { supportLevelLabels } from "@/features/sources/labels";
import type { SourceResolution } from "@/features/sources/view-model/registry-resolution";
import type {
  ProfileAccessPathDefinition,
  SelectedAccessPath,
  SourceOwnedSelectedAccessPath,
} from "@/lib/api/sources";

type AccessPathDetailsProps = {
  selectedAccessPath: SelectedAccessPath;
  resolution: SourceResolution;
};

export function AccessPathDetails({
  selectedAccessPath,
  resolution,
}: AccessPathDetailsProps) {
  if (selectedAccessPath.type === "profile_access_path") {
    return (
      <div className="grid gap-3 rounded-lg border p-3 text-sm">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <p className="font-medium">Profil-Zugriffspfad</p>
            <p className="text-xs text-muted-foreground">
              Die Source referenziert ein wiederverwendbares Source Profile und
              einen dort definierten Access Path.
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
        title="postingDiscovery"
        description="Deklarative source-weite Posting Discovery."
        value={accessPath.postingDiscovery}
        schemaRef={profileDslSchemaRefs.postingDiscoveryStep}
      />
      <OptionalSchemaValuePreview
        title="postingDetail"
        description="Optionale lazy Posting Detail Extraktion für eine konkrete Posting-Quelle."
        value={accessPath.postingDetail}
        schemaRef={profileDslSchemaRefs.postingDetailStep}
      />
    </div>
  );
}
