import type { ReactNode } from "react";

import { FunnelIcon, SearchIcon, XIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  diagnosticCountLabel,
  originEntries,
  profileKindEntries,
  sourceStatusEntries,
  type ProfileGridRow,
  type RegistryRowHealth,
  type SourceGridRow,
} from "@/features/sources/view-model/registry-view-model";
import type {
  SourceProfileKind,
  SourceRegistryDocumentOrigin,
  SourceStatus,
} from "@/lib/api/sources";

type RegistrySearchInputProps = {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  clearLabel: string;
};

export function RegistrySearchInput({
  value,
  onChange,
  placeholder,
  clearLabel,
}: RegistrySearchInputProps) {
  return (
    <InputGroup className="w-56 bg-background">
      <InputGroupAddon align="inline-start">
        <SearchIcon aria-hidden="true" />
      </InputGroupAddon>
      <InputGroupInput
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {value.length > 0 ? (
        <InputGroupAddon align="inline-end">
          <InputGroupButton
            aria-label={clearLabel}
            title={clearLabel}
            size="icon-xs"
            onClick={() => onChange("")}
          >
            <XIcon aria-hidden="true" />
          </InputGroupButton>
        </InputGroupAddon>
      ) : null}
    </InputGroup>
  );
}

type RegistryStateTone = "ready" | "warning" | "invalid";

const registryStateDotClasses: Record<RegistryStateTone, string> = {
  ready: "bg-success",
  warning: "bg-warning",
  invalid: "bg-destructive",
};

export function registryRowHealthClassName(health: RegistryRowHealth): string {
  switch (health) {
    case "invalid":
      return "bg-destructive/5 opacity-60 hover:bg-destructive/10";
    case "dependency_warning":
      return "bg-warning/5 hover:bg-warning/10";
    case "valid":
      return "";
  }
}

export function SourceRegistryStateDot({ row }: { row: SourceGridRow }) {
  const { label, tone } = registryStateDotState(
    row.health,
    row.diagnosticsCount,
  );

  return <RegistryStateDot label={label} tone={tone} />;
}

export function ProfileRegistryStateDot({ row }: { row: ProfileGridRow }) {
  const { label, tone } = registryStateDotState(
    row.health,
    row.diagnosticsCount,
  );

  return <RegistryStateDot label={label} tone={tone} />;
}

function registryStateDotState(
  health: RegistryRowHealth,
  diagnosticsCount: number,
): { label: string; tone: RegistryStateTone } {
  switch (health) {
    case "invalid":
      return {
        label:
          diagnosticsCount > 0
            ? `Ungültig · ${diagnosticCountLabel(diagnosticsCount)} · Details öffnen`
            : "Ungültig",
        tone: "invalid",
      };
    case "dependency_warning":
      return {
        label: `Abhängigkeit unvollständig · ${diagnosticCountLabel(diagnosticsCount)} · Details öffnen`,
        tone: "warning",
      };
    case "valid":
      return { label: "Alles OK", tone: "ready" };
  }
}

function RegistryStateDot({
  label,
  tone,
}: {
  label: string;
  tone: RegistryStateTone;
}) {
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      className="inline-flex size-4 shrink-0 items-center justify-center"
    >
      <span
        aria-hidden="true"
        className={`size-2 rounded-full ${registryStateDotClasses[tone]}`}
      />
    </span>
  );
}

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
          <FilterGroup title="Status">
            {sourceStatusEntries().map(([status, label]) => (
              <CheckboxFilterRow
                key={status}
                id={`source-status-${status}`}
                label={label}
                count={statusCounts[status] ?? 0}
                checked={selectedStatuses.includes(status)}
                onCheckedChange={(checked) => onStatusChange(status, checked)}
              />
            ))}
          </FilterGroup>
          <FilterGroup title="Origin">
            {originEntries().map(([origin, label]) => (
              <CheckboxFilterRow
                key={origin}
                id={`source-origin-${origin}`}
                label={label}
                count={originCounts[origin] ?? 0}
                checked={selectedOrigins.includes(origin)}
                onCheckedChange={(checked) => onOriginChange(origin, checked)}
              />
            ))}
          </FilterGroup>
          <FilterGroup title="Diagnosen">
            <CheckboxFilterRow
              id="source-diagnostics-only"
              label="Nur mit Diagnosen"
              checked={diagnosticsOnly}
              onCheckedChange={onDiagnosticsOnlyChange}
            />
          </FilterGroup>
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
          <FilterGroup title="Kind">
            {profileKindEntries().map(([kind, label]) => (
              <CheckboxFilterRow
                key={kind}
                id={`profile-kind-${kind}`}
                label={label}
                count={kindCounts[kind] ?? 0}
                checked={selectedKinds.includes(kind)}
                onCheckedChange={(checked) => onKindChange(kind, checked)}
              />
            ))}
          </FilterGroup>
          <FilterGroup title="Origin">
            {originEntries().map(([origin, label]) => (
              <CheckboxFilterRow
                key={origin}
                id={`profile-origin-${origin}`}
                label={label}
                count={originCounts[origin] ?? 0}
                checked={selectedOrigins.includes(origin)}
                onCheckedChange={(checked) => onOriginChange(origin, checked)}
              />
            ))}
          </FilterGroup>
          <FilterGroup title="Diagnosen">
            <CheckboxFilterRow
              id="profile-diagnostics-only"
              label="Nur mit Diagnosen"
              checked={diagnosticsOnly}
              onCheckedChange={onDiagnosticsOnlyChange}
            />
          </FilterGroup>
        </div>
      </PopoverContent>
    </Popover>
  );
}

type FilterGroupProps = {
  title: string;
  children: ReactNode;
};

function FilterGroup({ title, children }: FilterGroupProps) {
  return (
    <div className="grid gap-2">
      <div className="text-xs font-medium text-muted-foreground">{title}</div>
      <div className="grid gap-2">{children}</div>
    </div>
  );
}

type CheckboxFilterRowProps = {
  id: string;
  label: string;
  checked: boolean;
  count?: number;
  onCheckedChange: (checked: boolean) => void;
};

function CheckboxFilterRow({
  id,
  label,
  checked,
  count,
  onCheckedChange,
}: CheckboxFilterRowProps) {
  return (
    <div className="flex items-center gap-2.5">
      <Checkbox
        id={id}
        checked={checked}
        onCheckedChange={(nextChecked) => onCheckedChange(nextChecked === true)}
      />
      <Label
        htmlFor={id}
        className="flex grow items-center justify-between gap-1.5 font-normal"
      >
        <span>{label}</span>
        {typeof count === "number" ? (
          <span className="text-muted-foreground">{count}</span>
        ) : null}
      </Label>
    </div>
  );
}
