import { Drawer } from "@/components/ui/drawer";
import { SourceAddDrawer } from "@/features/sources/add/source/source-add-drawer";
import { SourceProfileTemplateDrawer } from "@/features/sources/add/source-profile/source-profile-template-drawer";
import type {
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

  return (
    <SourceProfileTemplateDrawer
      kind={kind}
      open={open}
      onOpenChange={onOpenChange}
    />
  );
}
