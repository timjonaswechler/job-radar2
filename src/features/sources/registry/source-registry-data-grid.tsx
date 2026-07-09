import { useMemo, useState } from "react";

import type { ColumnDef } from "@tanstack/react-table";
import { PlusIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { DataGridColumnHeader } from "@/components/reui/data-grid/data-grid-column-header";
import { DataGridPagination } from "@/components/reui/data-grid/data-grid-pagination";
import { DataGridScrollArea } from "@/components/reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "@/components/reui/data-grid/data-grid-table";
import { Button } from "@/components/ui/button";
import { useSourceRegistryGridState } from "@/features/sources/registry/registry-grid-state";
import { SourceEditDrawer } from "@/features/sources/edit/source/source-edit-drawer";
import { SourceDetailsDrawer } from "@/features/sources/registry/registry-details";
import { SourceFilterPopover } from "@/features/sources/registry/registry-toolbar";
import { RegistryGridShell } from "@/features/sources/registry/shared/registry-grid-shell";
import { RegistrySearchInput } from "@/features/sources/registry/shared/registry-search-input";
import {
  RegistryStateIndicator,
  registryRowHealthClassName,
} from "@/features/sources/registry/shared/registry-state-indicator";
import {
  type DiagnosticIndex,
  type SourceGridRow,
} from "@/features/sources/view-model/registry-view-model";
import {
  sourceStatusBadgeVariants,
  validationStateBadgeVariants,
} from "@/features/sources/status";
import type {
  RegistrySource,
  RegistrySourceProfile,
} from "@/lib/api/sources";

type SourceRegistryDataGridProps = {
  sources: RegistrySource[];
  profilesByKey: Map<string, RegistrySourceProfile>;
  diagnosticIndex: DiagnosticIndex;
  loading: boolean;
  onAdd: () => void;
  onUpdated?: () => Promise<unknown> | unknown;
};

export function SourceRegistryDataGrid({
  sources,
  profilesByKey,
  diagnosticIndex,
  loading,
  onAdd,
  onUpdated,
}: SourceRegistryDataGridProps) {
  const [editingSource, setEditingSource] = useState<RegistrySource | null>(null);
  const columns = useMemo<ColumnDef<SourceGridRow>[]>(
    () => [
      {
        accessorKey: "name",
        id: "name",
        header: ({ column }) => (
          <DataGridColumnHeader title="Quelle" visibility column={column} />
        ),
        cell: ({ row }) => (
          <div className="flex min-w-0 items-center gap-2">
            <RegistryStateIndicator
              health={row.original.health}
              diagnosticsCount={row.original.diagnosticsCount}
            />
            <div className="grid min-w-0 gap-0.5">
              <span className="truncate font-bold">{row.original.name}</span>
              <span className="truncate font-mono text-muted-foreground">
                {row.original.key}
              </span>
            </div>
          </div>
        ),
        size: 220,
        enableSorting: true,
        enableHiding: false,
        enableResizing: true,
      },
      {
        accessorKey: "statusLabel",
        id: "statusLabel",
        header: ({ column }) => (
          <DataGridColumnHeader title="Status" visibility column={column} />
        ),
        cell: ({ row }) => (
          <Badge variant={sourceStatusBadgeVariants[row.original.status]}>
            {row.original.statusLabel}
          </Badge>
        ),
        size: 110,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorKey: "validationStateLabel",
        id: "validationStateLabel",
        header: ({ column }) => (
          <DataGridColumnHeader title="Validierung" visibility column={column} />
        ),
        cell: ({ row }) => (
          <Badge variant={validationStateBadgeVariants[row.original.validationState]}>
            {row.original.validationStateLabel}
          </Badge>
        ),
        size: 130,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorKey: "supportLabel",
        id: "supportLabel",
        header: ({ column }) => (
          <DataGridColumnHeader title="Profil-Support" visibility column={column} />
        ),
        cell: ({ row }) => <Badge variant="outline">{row.original.supportLabel}</Badge>,
        size: 130,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorKey: "capabilitiesSummary",
        id: "capabilitiesSummary",
        header: ({ column }) => (
          <DataGridColumnHeader title="Fähigkeiten" visibility column={column} />
        ),
        cell: ({ row }) => (
          <span className="truncate text-muted-foreground">
            {row.original.capabilitiesSummary}
          </span>
        ),
        size: 170,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorKey: "originLabel",
        id: "originLabel",
        header: ({ column }) => (
          <DataGridColumnHeader title="Origin" visibility column={column} />
        ),
        cell: ({ row }) => (
          <Badge variant="secondary">{row.original.originLabel}</Badge>
        ),
        size: 110,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },

      {
        accessorKey: "configSummary",
        id: "configSummary",
        header: ({ column }) => (
          <DataGridColumnHeader title="Config" visibility column={column} />
        ),
        cell: ({ row }) => (
          <span className="truncate font-mono text-muted-foreground">
            {row.original.configSummary}
          </span>
        ),
        size: 160,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
    ],
    [],
  );

  const gridState = useSourceRegistryGridState({
    columns,
    diagnosticIndex,
    profilesByKey,
    sources,
  });

  const {
    activeFilterCount,
    diagnosticsOnly,
    filteredRows,
    originCounts,
    searchQuery,
    selectedOrigins,
    selectedRow,
    selectedStatuses,
    setDiagnosticsOnly,
    setSearchQuery,
    setSelectedRow,
    statusCounts,
    table,
    toggleOrigin,
    toggleStatus,
  } = gridState;

  return (
    <>
      <RegistryGridShell
        table={table}
        recordCount={filteredRows.length}
        isLoading={loading}
        loadingMessage="Quellen werden geladen…"
        emptyMessage="Keine Registry-Quellen gefunden."
        title="Registry-Quellen"
        description={
          <>
            Der Punkt vor dem Namen zeigt den Registry-Zustand. Bei Problemen
            Zeile anklicken, um Details im Drawer zu öffnen.
          </>
        }
        actions={
          <>
            <RegistrySearchInput
              value={searchQuery}
              onChange={setSearchQuery}
              label="Quellen suchen"
              name="source-registry-search"
              placeholder="Quellen suchen…"
              clearLabel="Quellensuche leeren"
            />
            <SourceFilterPopover
              selectedStatuses={selectedStatuses}
              selectedOrigins={selectedOrigins}
              diagnosticsOnly={diagnosticsOnly}
              statusCounts={statusCounts}
              originCounts={originCounts}
              activeFilterCount={activeFilterCount}
              onStatusChange={toggleStatus}
              onOriginChange={toggleOrigin}
              onDiagnosticsOnlyChange={setDiagnosticsOnly}
            />
            <Button type="button" onClick={onAdd}>
              <PlusIcon data-icon="inline-start" aria-hidden="true" />
              Quelle hinzufügen
            </Button>
          </>
        }
        pagination={
          <DataGridPagination
            sizes={[5, 10, 25, 50]}
            rowsPerPageLabel="Zeilen pro Seite"
            info="{from}–{to} von {count}"
            previousPageLabel="Vorherige Quellen-Seite"
            nextPageLabel="Nächste Quellen-Seite"
          />
        }
        onRowClick={setSelectedRow}
        rowClassName={(row) => registryRowHealthClassName(row.health)}
        className="px-1"
        actionsClassName="items-baseline"
      >
        <DataGridScrollArea>
          <DataGridTable />
        </DataGridScrollArea>
      </RegistryGridShell>

      <SourceDetailsDrawer
        row={selectedRow}
        profilesByKey={profilesByKey}
        diagnostics={
          selectedRow
            ? (diagnosticIndex.bySourceKey.get(selectedRow.key) ?? [])
            : []
        }
        open={selectedRow !== null}
        onEdit={(source) => {
          setEditingSource(source);
          setSelectedRow(null);
        }}
        onUpdated={onUpdated}
        onOpenChange={(open) => {
          if (!open) setSelectedRow(null);
        }}
      />

      <SourceEditDrawer
        source={editingSource}
        profilesByKey={profilesByKey}
        open={editingSource !== null}
        onUpdated={onUpdated}
        onOpenChange={(open) => {
          if (!open) setEditingSource(null);
        }}
      />
    </>
  );
}
