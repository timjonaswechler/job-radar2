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
  originEntries,
  profileKindEntries,
  sourceStatusEntries,
} from "@/features/sources/view-model/registry-view-model";
import type {
  SourceProfileKind,
  SourceRegistryDocumentOrigin,
  SourceStatus,
} from "@/lib/api/sources";

type SourceFilterPopoverProps = {
  selectedStatuses: SourceStatus[];
  selectedOrigins: SourceRegistryDocumentOrigin[];
  diagnosticsOnly: boolean;
  statusCounts: Record<SourceStatus, number>;
  originCounts: Record<SourceRegistryDocumentOrigin, number>;
  activeFilterCount: number;
  onStatusChange: (status: SourceStatus, checked: boolean) => void;
  onOriginChange: (
    origin: SourceRegistryDocumentOrigin,
    checked: boolean,
  ) => void;
  onDiagnosticsOnlyChange: (checked: boolean) => void;
};

export function SourceFilterPopover({
  selectedStatuses,
  selectedOrigins,
  diagnosticsOnly,
  statusCounts,
  originCounts,
  activeFilterCount,
  onStatusChange,
  onOriginChange,
  onDiagnosticsOnlyChange,
}: SourceFilterPopoverProps) {
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
          <RegistryFilterFields title="Status">
            {sourceStatusEntries().map(([status, label]) => (
              <RegistryCheckboxFilterRow
                key={status}
                id={`source-status-${status}`}
                label={label}
                count={statusCounts[status] ?? 0}
                checked={selectedStatuses.includes(status)}
                onCheckedChange={(checked) => onStatusChange(status, checked)}
              />
            ))}
          </RegistryFilterFields>
          <RegistryFilterFields title="Origin">
            {originEntries().map(([origin, label]) => (
              <RegistryCheckboxFilterRow
                key={origin}
                id={`source-origin-${origin}`}
                label={label}
                count={originCounts[origin] ?? 0}
                checked={selectedOrigins.includes(origin)}
                onCheckedChange={(checked) => onOriginChange(origin, checked)}
              />
            ))}
          </RegistryFilterFields>
          <RegistryFilterFields title="Diagnosen">
            <RegistryCheckboxFilterRow
              id="source-diagnostics-only"
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

type ProfileFilterPopoverProps = {
  selectedKinds: SourceProfileKind[];
  selectedOrigins: SourceRegistryDocumentOrigin[];
  diagnosticsOnly: boolean;
  kindCounts: Record<SourceProfileKind, number>;
  originCounts: Record<SourceRegistryDocumentOrigin, number>;
  activeFilterCount: number;
  onKindChange: (kind: SourceProfileKind, checked: boolean) => void;
  onOriginChange: (
    origin: SourceRegistryDocumentOrigin,
    checked: boolean,
  ) => void;
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
            {originEntries().map(([origin, label]) => (
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
