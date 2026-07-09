import { XIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { ProfileAccessPathRow } from "@/features/sources/registry/access-path-details";
import { DetailRow } from "@/features/sources/registry/detail-row";
import { InlineDiagnostics } from "@/features/sources/registry/registry-diagnostics";
import { profileDslSchemaRefs } from "@/features/sources/shared/profile-dsl-schema-catalog";
import { OptionalSchemaValuePreview } from "@/features/sources/shared/schema-value-table";
import {
  detectionEvidenceKindLabels,
  originLabels,
  profileKindLabels,
  supportEvidenceKindLabels,
  supportLevelLabels,
} from "@/features/sources/labels";
import type { ProfileGridRow } from "@/features/sources/view-model/registry-view-model";
import type {
  RegistrySourceProfile,
  StructuredDiagnostic,
} from "@/lib/api/sources";

type ProfileDetailsDrawerProps = {
  row: ProfileGridRow | null;
  diagnostics: StructuredDiagnostic[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function ProfileDetailsDrawer({
  row,
  diagnostics,
  open,
  onOpenChange,
}: ProfileDetailsDrawerProps) {
  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction="right">
      {row ? (
        <DrawerContent
          className="h-full data-[vaul-drawer-direction=right]:w-[min(calc(100vw-115px),960px)]
      data-[vaul-drawer-direction=right]:sm:max-w-none"
        >
          <DrawerHeader className="border-b pr-12">
            <DrawerTitle>{row.name}</DrawerTitle>
            <DrawerDescription>
              Profil-Key <code>{row.key}</code> · {row.kindLabel} ·{" "}
              deklarierter Support: {row.supportLabel} · {row.originLabel}
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
          <div className="min-h-0 overflow-y-auto px-4 pb-4">
            <ProfileDetails profile={row.profile} diagnostics={diagnostics} />
          </div>
        </DrawerContent>
      ) : null}
    </Drawer>
  );
}

type EvidenceBadgeSectionProps<TKind extends string> = {
  title: string;
  description: string;
  emptyLabel: string;
  evidence: Array<{
    kind: TKind;
    reference?: string;
    message?: string;
    summary?: string;
  }>;
  labelForKind: (kind: TKind) => string;
};

function EvidenceBadgeSection<TKind extends string>({
  title,
  description,
  emptyLabel,
  evidence,
  labelForKind,
}: EvidenceBadgeSectionProps<TKind>) {
  return (
    <section className="grid gap-2 rounded-lg border bg-muted/30 p-3">
      <div className="grid gap-1">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          {title}
        </h3>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      {evidence.length ? (
        <div className="flex flex-wrap gap-1.5">
          {evidence.map((item, index) => (
            <Badge
              key={`${item.kind}-${item.reference ?? item.message ?? index}`}
              variant="secondary"
              title={item.summary ?? item.message ?? item.reference}
            >
              {labelForKind(item.kind)}
            </Badge>
          ))}
        </div>
      ) : (
        <span className="text-xs text-muted-foreground">{emptyLabel}</span>
      )}
    </section>
  );
}

type ProfileDetailsProps = {
  profile: RegistrySourceProfile;
  diagnostics: StructuredDiagnostic[];
};

function ProfileDetails({ profile, diagnostics }: ProfileDetailsProps) {
  const accessPaths = [...profile.document.accessPaths].sort((left, right) =>
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
      {profile.document.diagnostics?.length ? (
        <InlineDiagnostics
          title="Im Profil gespeicherte Diagnosen"
          diagnostics={profile.document.diagnostics}
        />
      ) : null}

      <dl className="grid gap-3 rounded-lg border bg-muted/30 p-3 sm:grid-cols-2">
        <DetailRow label="Profil-Key" value={profile.document.key} mono />
        <DetailRow label="Name" value={profile.document.name} />
        <DetailRow
          label="Kind"
          value={profileKindLabels[profile.document.kind]}
        />
        <DetailRow
          label="Deklarierter Support"
          value={supportLevelLabels[profile.document.support.level]}
        />
        <DetailRow label="Ursprung" value={originLabels[profile.origin]} />
        <DetailRow label="Registry-Dokument" value={profile.path} mono />
      </dl>

      {profile.document.description ? (
        <p className="text-muted-foreground">{profile.document.description}</p>
      ) : null}
      <div className="flex flex-wrap gap-1">
        {profile.document.support.knownIssues?.map((issue, index) => (
          <Badge key={`${issue.message}-${index}`} variant="warning-light">
            {issue.scope ? `${issue.scope}: ` : ""}
            {issue.message}
          </Badge>
        ))}
      </div>

      <EvidenceBadgeSection
        title="Support-Evidenz"
        description="Deklarierte Support-Evidenz beschreibt die Einschätzung zum Profil. Die konkrete Nutzbarkeit wird über Source Live Checks geprüft."
        emptyLabel="Keine Support-Evidenz deklariert."
        evidence={profile.document.support.evidence ?? []}
        labelForKind={(kind) => supportEvidenceKindLabels[kind]}
      />
      <EvidenceBadgeSection
        title="Detection-Evidenz"
        description="Detection-Evidenz gehört zu detect.evidence und ist getrennt von Support-Evidenz. URL bleibt hier gültige Detection-Evidenz."
        emptyLabel="Keine Detection-Evidenz deklariert."
        evidence={profile.document.detect?.evidence ?? []}
        labelForKind={(kind) => detectionEvidenceKindLabels[kind]}
      />

      <OptionalSchemaValuePreview
        title="support"
        description="Support Level, bekannte Einschränkungen und Evidenz des Source Profile."
        value={profile.document.support}
        schemaRef={profileDslSchemaRefs.supportMetadata}
      />
      <OptionalSchemaValuePreview
        title="Profil sourceConfigSchema"
        description="Schema-Anteil, der für alle Access Paths dieses Profils gilt."
        value={profile.document.sourceConfigSchema}
      />
      <OptionalSchemaValuePreview
        title="Detection-Regeln"
        description="Regeln, wie dieses Profil bei eingereichten URLs eine Source Proposal erzeugt."
        value={profile.document.detect}
        schemaRef={profileDslSchemaRefs.detection}
      />

      <div className="grid gap-2">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          Access Paths
        </h3>
        {accessPaths.map((accessPath) => (
          <ProfileAccessPathRow key={accessPath.key} accessPath={accessPath} />
        ))}
      </div>
    </div>
  );
}
