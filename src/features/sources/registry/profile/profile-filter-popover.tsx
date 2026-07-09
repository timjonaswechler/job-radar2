import { FunnelIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  RegistryCheckboxFilterRow,
  RegistryFilterFields,
} from "@/features/sources/registry/shared/registry-filter-fields";
import {
  profileKindEntries,
  profileOriginEntries,
} from "@/features/sources/view-model/profile-grid-model";
import type {
  SourceProfileKind,
  SourceRegistryDocumentOrigin,
} from "@/lib/api/sources";

type ProfileFilterPopoverProps = {
  selectedKinds: SourceProfileKind[];
  selectedOrigins: SourceRegistryDocumentOrigin[];
  diagnosticsOnly: boolean;
  kindCounts: Record<SourceProfileKind, number>;
  originCounts: Record<SourceRegistryDocumentOrigin, number>;
  activeFilterCount: number;
  onKindChange: (kind: SourceProfileKind, checked: boolean) => void;
  onOriginChange: (origin: SourceRegistryDocumentOrigin, checked: boolean) => void;
  onDiagnosticsOnlyChange: (checked: boolean) => void;
};

export function ProfileFilterPopover({
  selectedKinds,
  selectedOrigins,
  diagnosticsOnly,
  kindCounts,
  originCounts,
  activeFilterCount,
  onKindChange,
  onOriginChange,
  onDiagnosticsOnlyChange,
}: ProfileFilterPopoverProps) {
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button type="button" variant="outline">
            <FunnelIcon data-icon="inline-start" aria-hidden="true" />
            Filter
            {activeFilterCount > 0 ? (
              <Badge size="sm" variant="info-outline">
                {activeFilterCount}
              </Badge>
            ) : null}
          </Button>
        }
      />
      <PopoverContent className="w-72" align="start">
        <div className="grid gap-4">
          <RegistryFilterFields title="Kind">
            {profileKindEntries().map(([kind, label]) => (
              <RegistryCheckboxFilterRow
                key={kind}
                id={`profile-kind-${kind}`}
                label={label}
                count={kindCounts[kind] ?? 0}
                checked={selectedKinds.includes(kind)}
                onCheckedChange={(checked) => onKindChange(kind, checked)}
              />
            ))}
          </RegistryFilterFields>
          <RegistryFilterFields title="Origin">
            {profileOriginEntries().map(([origin, label]) => (
              <RegistryCheckboxFilterRow
                key={origin}
                id={`profile-origin-${origin}`}
                label={label}
                count={originCounts[origin] ?? 0}
                checked={selectedOrigins.includes(origin)}
                onCheckedChange={(checked) => onOriginChange(origin, checked)}
              />
            ))}
          </RegistryFilterFields>
          <RegistryFilterFields title="Diagnosen">
            <RegistryCheckboxFilterRow
              id="profile-diagnostics-only"
              label="Nur mit Diagnosen"
              checked={diagnosticsOnly}
              onCheckedChange={onDiagnosticsOnlyChange}
            />
          </RegistryFilterFields>
        </div>
      </PopoverContent>
    </Popover>
  );
}
