import { Badge } from "@/components/reui/badge";
import { DetailRow } from "@/features/sources/registry/detail-row";
import { ProfileAccessPathDetails } from "@/features/sources/registry/profile/profile-access-path-details";
import {
  ProfileDetectionEvidenceSection,
  ProfileSupportEvidenceSection,
} from "@/features/sources/registry/profile/profile-evidence-section";
import { InlineDiagnostics } from "@/features/sources/registry/diagnostics/inline-diagnostics";
import { sourceBehaviorSchemaRefs } from "@/features/sources/shared/source-schema-catalog";
import { OptionalSchemaValuePreview } from "@/features/sources/shared/schema-value-table";
import {
  originLabels,
  profileKindLabels,
  supportLevelLabels,
} from "@/features/sources/labels";
import type {
  InstalledProfileWithDefinition,
  StructuredDiagnostic,
} from "@/lib/api/sources";

type ProfileDetailsProps = {
  profile: InstalledProfileWithDefinition;
  diagnostics: StructuredDiagnostic[];
};

export function ProfileDetails({
  profile,
  diagnostics,
}: ProfileDetailsProps) {
  const accessPaths = [...profile.definition.accessPaths].sort((left, right) =>
    left.key.localeCompare(right.key, "de"),
  );

  return (
    <div className="grid gap-4 py-4 text-sm">
      {diagnostics.length ? (
        <InlineDiagnostics
          title="Diagnosen zu diesem Source Profile"
          diagnostics={diagnostics}
        />
      ) : null}
      <dl className="grid gap-3 rounded-lg border bg-muted/30 p-3 sm:grid-cols-2">
        <DetailRow label="Profil-Key" value={profile.definition.key} mono />
        <DetailRow label="Name" value={profile.definition.name} />
        <DetailRow
          label="Kind"
          value={profileKindLabels[profile.definition.kind]}
        />
        <DetailRow
          label="Deklarierter Support"
          value={supportLevelLabels[profile.definition.support.level]}
        />
        <DetailRow label="Ursprung" value={originLabels[profile.origin]} />
        <DetailRow label="Registry-Dokument" value={profile.fileName} mono />
      </dl>

      {profile.definition.description ? (
        <p className="text-muted-foreground">{profile.definition.description}</p>
      ) : null}
      <div className="flex flex-wrap gap-1">
        {profile.definition.support.knownIssues?.map((issue, index) => (
          <Badge key={`${issue.message}-${index}`} variant="warning-light">
            {issue.scope ? `${issue.scope}: ` : ""}
            {issue.message}
          </Badge>
        ))}
      </div>

      <ProfileSupportEvidenceSection
        evidence={profile.definition.support.evidence ?? []}
      />
      <ProfileDetectionEvidenceSection
        evidence={profile.definition.detection?.evidence ?? []}
      />

      <OptionalSchemaValuePreview
        title="support"
        description="Support Level, bekannte Einschränkungen und Evidenz des Source Profile."
        value={profile.definition.support}
        schemaRef={sourceBehaviorSchemaRefs.supportMetadata}
      />
      <OptionalSchemaValuePreview
        title="Profil sourceConfigSchema"
        description="Schema-Anteil, der für alle Access Paths dieses Profils gilt."
        value={profile.definition.sourceConfigSchema}
      />
      <OptionalSchemaValuePreview
        title="Detection-Regeln"
        description="Regeln, wie dieses Profil bei eingereichten URLs eine Source Proposal erzeugt."
        value={profile.definition.detection}
        schemaRef={sourceBehaviorSchemaRefs.detection}
      />

      <div className="grid gap-2">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          Access Paths
        </h3>
        {accessPaths.map((accessPath) => (
          <ProfileAccessPathDetails
            key={accessPath.key}
            accessPath={accessPath}
          />
        ))}
      </div>
    </div>
  );
}
