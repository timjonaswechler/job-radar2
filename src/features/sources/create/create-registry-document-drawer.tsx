import { Drawer } from "@/components/ui/drawer";
import { SourceProfileTemplateDrawer } from "@/features/sources/create/source-profile/source-profile-template-drawer";
import { SourceCreateDrawer } from "@/features/sources/create/source/source-create-drawer";
import type {
  RegistrySource,
  RegistrySourceProfile,
  SourceRegistryDocumentKind,
} from "@/lib/api/sources";

type CreateRegistryDocumentDrawerProps = {
  kind: SourceRegistryDocumentKind | null;
  open: boolean;
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  onCreated?: () => Promise<unknown> | unknown;
  onOpenChange: (open: boolean) => void;
};

export function CreateRegistryDocumentDrawer({
  kind,
  open,
  profiles,
  sources,
  onCreated,
  onOpenChange,
}: CreateRegistryDocumentDrawerProps) {
  if (!kind) {
    return <Drawer open={open} onOpenChange={onOpenChange} direction="right" />;
  }

  if (kind === "source") {
    return (
      <SourceCreateDrawer
        open={open}
        profiles={profiles}
        sources={sources}
        onCreated={onCreated}
        onOpenChange={onOpenChange}
      />
    );
  }

  return (
    <SourceProfileTemplateDrawer
      kind={kind}
      open={open}
      onOpenChange={onOpenChange}
    />
  );
}
