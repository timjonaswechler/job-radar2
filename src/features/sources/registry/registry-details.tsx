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
import {
  AccessPathDetails,
  ProfileAccessPathRow,
} from "@/features/sources/registry/access-path-details";
import { DetailRow } from "@/features/sources/registry/detail-row";
import {
  JsonPreview,
  OptionalJsonPreview,
} from "@/features/sources/shared/json-preview";
import { InlineDiagnostics } from "@/features/sources/registry/registry-diagnostics";
import {
  originLabels,
  profileKindLabels,
  supportLevelLabels,
  validationStateLabels,
} from "@/features/sources/labels";
import {
  resolveSource,
  type ProfileGridRow,
  type SourceGridRow,
} from "@/features/sources/view-model/registry-view-model";
import { sourceStatusLabels } from "@/features/sources/status";
import type {
  RegistrySource,
  RegistrySourceProfile,
  StructuredDiagnostic,
} from "@/lib/api/sources";

type SourceDetailsDrawerProps = {
  row: SourceGridRow | null;
  profilesByKey: Map<string, RegistrySourceProfile>;
  diagnostics: StructuredDiagnostic[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function SourceDetailsDrawer({
  row,
  profilesByKey,
  diagnostics,
  open,
  onOpenChange,
}: SourceDetailsDrawerProps) {
  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction="right">
      {row ? (
        <DrawerContent
          className="h-full data-[vaul-drawer-direction=right]:w-[min(calc(100vw_-_115px),960px)]
        data-[vaul-drawer-direction=right]:sm:max-w-none"
        >
          <DrawerHeader className="border-b pr-12">
            <DrawerTitle>{row.name}</DrawerTitle>
            <DrawerDescription>
              Source Key <code>{row.key}</code> · {row.statusLabel} ·{" "}
              {row.validationStateLabel} · {row.originLabel}
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
            <SourceDetails
              source={row.source}
              profilesByKey={profilesByKey}
              diagnostics={diagnostics}
            />
          </div>
        </DrawerContent>
      ) : null}
    </Drawer>
  );
}

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
        <DrawerContent className="h-full sm:max-w-xl lg:max-w-2xl">
          <DrawerHeader className="border-b pr-12">
            <DrawerTitle>{row.name}</DrawerTitle>
            <DrawerDescription>
              Profil-Key <code>{row.key}</code> · {row.kindLabel} ·{" "}
              {row.supportLabel} · {row.originLabel}
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

type SourceDetailsProps = {
  source: RegistrySource;
  profilesByKey: Map<string, RegistrySourceProfile>;
  diagnostics: StructuredDiagnostic[];
};

function SourceDetails({
  source,
  profilesByKey,
  diagnostics,
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

      <dl className="grid gap-3 rounded-lg border bg-muted/30 p-3 sm:grid-cols-2">
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
          label="Support"
          value={
            resolution.supportLevel
              ? supportLevelLabels[resolution.supportLevel]
              : "—"
          }
        />
        <DetailRow label="Ursprung" value={originLabels[source.origin]} />
        <DetailRow label="Registry-Dokument" value={source.path} mono />
      </dl>

      <AccessPathDetails
        selectedAccessPath={selectedAccessPath}
        resolution={resolution}
      />

      <JsonPreview
        title="sourceConfig"
        description="Stabile Zugriffskonfiguration der Source. Search Request Kriterien gehören nicht hierher."
        value={source.document.sourceConfig}
        defaultOpen
      />
      <JsonPreview
        title="Effektives sourceConfigSchema"
        description="Profil- und Access-Path-Schema, wie die Registry sie für diese Source zusammenführt."
        value={resolution.effectiveSourceConfigSchema}
      />
      <OptionalJsonPreview
        title="sourceOverrides"
        description="Kontrollierte Source-spezifische Verhaltensänderungen für den ausgewählten Profilpfad."
        value={source.document.sourceOverrides}
      />
      <OptionalJsonPreview
        title="sourceSupport"
        description="Support-Metadaten für Source-owned Access Paths."
        value={source.document.sourceSupport}
      />
      <OptionalJsonPreview
        title="Source-Diagnosen im Dokument"
        description="Im Source-Dokument gespeicherte strukturierte Diagnosen."
        value={source.document.diagnostics}
      />
    </div>
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
          label="Support"
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

      <OptionalJsonPreview
        title="support"
        description="Support Level, bekannte Einschränkungen und Evidenz des Source Profile."
        value={profile.document.support}
      />
      <OptionalJsonPreview
        title="Profil sourceConfigSchema"
        description="Schema-Anteil, der für alle Access Paths dieses Profils gilt."
        value={profile.document.sourceConfigSchema}
      />
      <OptionalJsonPreview
        title="Detection-Regeln"
        description="Regeln, wie dieses Profil bei eingereichten URLs eine Source Proposal erzeugt."
        value={profile.document.detect}
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
