import { PencilIcon, XIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { SourceDetails } from "@/features/sources/registry/source/source-details";
import type { SourceGridRow } from "@/features/sources/view-model/source-grid-model";
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
  onEdit?: (source: RegistrySource) => void;
  onUpdated?: () => Promise<unknown> | unknown;
  onOpenChange: (open: boolean) => void;
};

export function SourceDetailsDrawer({
  row,
  profilesByKey,
  diagnostics,
  open,
  onEdit,
  onUpdated,
  onOpenChange,
}: SourceDetailsDrawerProps) {
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
              Source Key <code>{row.key}</code> · {row.statusLabel} ·{" "}
              {row.validationStateLabel} · {row.originLabel}
            </DrawerDescription>
            {row.source.origin === "custom" &&
            row.source.document.selectedAccessPath.type === "profile_access_path" ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="absolute top-5 right-16"
                onClick={() => onEdit?.(row.source)}
              >
                <PencilIcon data-icon="inline-start" aria-hidden="true" />
                Bearbeiten
              </Button>
            ) : null}
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
              onUpdated={onUpdated}
            />
          </div>
        </DrawerContent>
      ) : null}
    </Drawer>
  );
}
