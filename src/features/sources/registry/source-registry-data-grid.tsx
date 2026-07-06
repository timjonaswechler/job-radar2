import { useMemo } from "react";

import type { ColumnDef } from "@tanstack/react-table";
import { PlusIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { DataGrid } from "@/components/reui/data-grid/data-grid";
import { DataGridColumnHeader } from "@/components/reui/data-grid/data-grid-column-header";
import { DataGridPagination } from "@/components/reui/data-grid/data-grid-pagination";
import { DataGridScrollArea } from "@/components/reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "@/components/reui/data-grid/data-grid-table";
import {
  Frame,
  FrameDescription,
  FrameFooter,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import { useSourceRegistryGridState } from "@/features/sources/registry/registry-grid-state";
import { SourceDetailsDrawer } from "@/features/sources/registry/registry-details";
import {
  RegistrySearchInput,
  SourceFilterPopover,
  SourceRegistryStateDot,
  registryRowHealthClassName,
} from "@/features/sources/registry/registry-toolbar";
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
};

export function SourceRegistryDataGrid({
  sources,
  profilesByKey,
  diagnosticIndex,
  loading,
  onAdd,
}: SourceRegistryDataGridProps) {
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
            <SourceRegistryStateDot row={row.original} />
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
          <DataGridColumnHeader title="Support" visibility column={column} />
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
      <DataGrid
        table={table}
        recordCount={filteredRows.length}
        isLoading={loading}
        loadingMessage="Quellen werden geladen…"
        emptyMessage="Keine Registry-Quellen gefunden."
        onRowClick={(row) => setSelectedRow(row)}
        tableClassNames={{
          bodyRow: (row) => registryRowHealthClassName(row.health),
        }}
        tableLayout={{
          columnsPinnable: true,
          columnsResizable: false,
          columnsMovable: true,
          columnsVisibility: true,
        }}
      >
        <Frame className="px-1 w-full" stacked dense>
          <FrameHeader className="gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div className="grid gap-1.5">
              <FrameTitle>Registry-Quellen</FrameTitle>
              <FrameDescription>
                Der Punkt vor dem Namen zeigt den Registry-Zustand. Bei
                Problemen Zeile anklicken, um Details im Drawer zu öffnen.
              </FrameDescription>
            </div>
            <div className="flex flex-wrap items-baseline gap-2.5">
              <RegistrySearchInput
                value={searchQuery}
                onChange={setSearchQuery}
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
            </div>
          </FrameHeader>
          <FramePanel className="p-0 shadow-none">
            <DataGridScrollArea>
              <DataGridTable />
            </DataGridScrollArea>
          </FramePanel>
          <FrameFooter className="py-1.5 pr-2 pl-2.5">
            <DataGridPagination
              sizes={[5, 10, 25, 50]}
              rowsPerPageLabel="Zeilen pro Seite"
              info="{from}–{to} von {count}"
              previousPageLabel="Vorherige Quellen-Seite"
              nextPageLabel="Nächste Quellen-Seite"
            />
          </FrameFooter>
        </Frame>
      </DataGrid>

      <SourceDetailsDrawer
        row={selectedRow}
        profilesByKey={profilesByKey}
        diagnostics={
          selectedRow
            ? (diagnosticIndex.bySourceKey.get(selectedRow.key) ?? [])
            : []
        }
        open={selectedRow !== null}
        onOpenChange={(open) => {
          if (!open) setSelectedRow(null);
        }}
      />
    </>
  );
}
