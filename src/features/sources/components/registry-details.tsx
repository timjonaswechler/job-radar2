import { XIcon } from "lucide-react";

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
} from "@/features/sources/components/access-path-details";
import { DetailRow } from "@/features/sources/components/detail-row";
import {
  JsonPreview,
  OptionalJsonPreview,
} from "@/features/sources/components/json-preview";
import { InlineDiagnostics } from "@/features/sources/components/registry-diagnostics";
import { originLabels, profileKindLabels } from "@/features/sources/labels";
import {
  resolveSource,
  type ProfileGridRow,
  type SourceGridRow,
} from "@/features/sources/registry-view-model";
import { sourceStatusLabels } from "@/features/sources/status";
import type {
  AdapterMetadata,
  RegistrySource,
  RegistrySourceProfile,
  SourceRegistryDiagnostic,
} from "@/lib/api/sources";

type SourceDetailsDrawerProps = {
  row: SourceGridRow | null;
  profilesByKey: Map<string, RegistrySourceProfile>;
  adaptersByKey: Map<string, AdapterMetadata>;
  diagnostics: SourceRegistryDiagnostic[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function SourceDetailsDrawer({
  row,
  profilesByKey,
  adaptersByKey,
  diagnostics,
  open,
  onOpenChange,
}: SourceDetailsDrawerProps) {
  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction="right">
      {row ? (
        <DrawerContent className="h-full sm:max-w-xl lg:max-w-2xl">
          <DrawerHeader className="border-b pr-12">
            <DrawerTitle>{row.name}</DrawerTitle>
            <DrawerDescription>
              Source Key <code>{row.key}</code> · {row.statusLabel} ·{" "}
              {row.originLabel}
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
              adaptersByKey={adaptersByKey}
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
  adaptersByKey: Map<string, AdapterMetadata>;
  diagnostics: SourceRegistryDiagnostic[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function ProfileDetailsDrawer({
  row,
  adaptersByKey,
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
              {row.originLabel}
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
            <ProfileDetails
              profile={row.profile}
              adaptersByKey={adaptersByKey}
              diagnostics={diagnostics}
            />
          </div>
        </DrawerContent>
      ) : null}
    </Drawer>
  );
}

type SourceDetailsProps = {
  source: RegistrySource;
  profilesByKey: Map<string, RegistrySourceProfile>;
  adaptersByKey: Map<string, AdapterMetadata>;
  diagnostics: SourceRegistryDiagnostic[];
};

function SourceDetails({
  source,
  profilesByKey,
  adaptersByKey,
  diagnostics,
}: SourceDetailsProps) {
  const selectedAccessPath = source.document.selectedAccessPath;
  const resolution = resolveSource(source, profilesByKey, adaptersByKey);

  return (
    <div className="grid gap-4 py-4 text-sm">
      {diagnostics.length ? (
        <InlineDiagnostics
          title="Diagnosen zu dieser Quelle"
          diagnostics={diagnostics}
        />
      ) : null}

      <dl className="grid gap-3 rounded-lg border bg-muted/30 p-3 sm:grid-cols-2">
        <DetailRow label="Source Key" value={source.document.key} mono />
        <DetailRow label="Name" value={source.document.name} />
        <DetailRow
          label="Status"
          value={sourceStatusLabels[source.document.status]}
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
        description="Konkrete Konfiguration des Source-Dokuments."
        value={source.document.sourceConfig}
        defaultOpen
      />
      <JsonPreview
        title="Effektives sourceConfigSchema"
        description="Profil- und Zugriffspfad-Schema, wie die Registry sie für diese Quelle zusammenführt."
        value={resolution.effectiveSourceConfigSchema}
      />
    </div>
  );
}

type ProfileDetailsProps = {
  profile: RegistrySourceProfile;
  adaptersByKey: Map<string, AdapterMetadata>;
  diagnostics: SourceRegistryDiagnostic[];
};

function ProfileDetails({
  profile,
  adaptersByKey,
  diagnostics,
}: ProfileDetailsProps) {
  const accessPaths = [...profile.document.accessPaths].sort((left, right) =>
    left.key.localeCompare(right.key, "de"),
  );

  return (
    <div className="grid gap-4 py-4 text-sm">
      {diagnostics.length ? (
        <InlineDiagnostics
          title="Diagnosen zu diesem Profil"
          diagnostics={diagnostics}
        />
      ) : null}

      <dl className="grid gap-3 rounded-lg border bg-muted/30 p-3 sm:grid-cols-2">
        <DetailRow label="Profil-Key" value={profile.document.key} mono />
        <DetailRow label="Name" value={profile.document.name} />
        <DetailRow
          label="Kind"
          value={profileKindLabels[profile.document.kind]}
        />
        <DetailRow label="Ursprung" value={originLabels[profile.origin]} />
        <DetailRow label="Registry-Dokument" value={profile.path} mono />
      </dl>

      <OptionalJsonPreview
        title="Profil sourceConfigSchema"
        description="Schema-Anteil, der für alle Zugriffspfade dieses Profils gilt."
        value={profile.document.sourceConfigSchema}
      />
      <OptionalJsonPreview
        title="Detection-Regeln"
        description="Hinweise, wie dieses Profil bei eingereichten URLs erkannt wird."
        value={profile.document.detect}
      />
      <OptionalJsonPreview
        title="Identity-Kandidaten"
        description="Templates für vorgeschlagene Source Keys und Namen."
        value={profile.document.identity}
      />

      <div className="grid gap-2">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          Zugriffspfade
        </h3>
        {accessPaths.map((accessPath) => (
          <ProfileAccessPathRow
            key={accessPath.key}
            accessPath={accessPath}
            adapter={adaptersByKey.get(accessPath.adapterKey)}
          />
        ))}
      </div>
    </div>
  );
}
