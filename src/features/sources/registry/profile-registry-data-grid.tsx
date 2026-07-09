import { useMemo } from "react";

import type { ColumnDef } from "@tanstack/react-table";
import { PlusIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { DataGridColumnHeader } from "@/components/reui/data-grid/data-grid-column-header";
import { DataGridPagination } from "@/components/reui/data-grid/data-grid-pagination";
import { DataGridScrollArea } from "@/components/reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "@/components/reui/data-grid/data-grid-table";
import { Button } from "@/components/ui/button";
import { useProfileRegistryGridState } from "@/features/sources/registry/registry-grid-state";
import { ProfileDetailsDrawer } from "@/features/sources/registry/registry-details";
import { ProfileFilterPopover } from "@/features/sources/registry/registry-toolbar";
import { RegistryGridShell } from "@/features/sources/registry/shared/registry-grid-shell";
import { RegistrySearchInput } from "@/features/sources/registry/shared/registry-search-input";
import {
  RegistryStateIndicator,
  registryRowHealthClassName,
} from "@/features/sources/registry/shared/registry-state-indicator";
import {
  type DiagnosticIndex,
  type ProfileGridRow,
} from "@/features/sources/view-model/registry-view-model";
import type { RegistrySourceProfile } from "@/lib/api/sources";

type ProfileRegistryDataGridProps = {
  profiles: RegistrySourceProfile[];
  diagnosticIndex: DiagnosticIndex;
  loading: boolean;
  onAdd: () => void;
};

export function ProfileRegistryDataGrid({
  profiles,
  diagnosticIndex,
  loading,
  onAdd,
}: ProfileRegistryDataGridProps) {
  const columns = useMemo<ColumnDef<ProfileGridRow>[]>(
    () => [
      {
        accessorKey: "name",
        id: "name",
        header: ({ column }) => (
          <DataGridColumnHeader title="Profil" visibility column={column} />
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
        size: 230,
        enableSorting: true,
        enableHiding: false,
        enableResizing: true,
      },
      {
        accessorKey: "supportLabel",
        id: "supportLabel",
        header: ({ column }) => (
          <DataGridColumnHeader title="Deklarierter Support" visibility column={column} />
        ),
        cell: ({ row }) => <Badge variant="outline">{row.original.supportLabel}</Badge>,
        size: 170,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorKey: "supportEvidenceSummary",
        id: "supportEvidenceSummary",
        header: ({ column }) => (
          <DataGridColumnHeader title="Support-Evidenz" visibility column={column} />
        ),
        cell: ({ row }) =>
          row.original.supportEvidenceLabels.length ? (
            <div className="flex min-w-0 flex-wrap gap-1">
              {row.original.supportEvidenceLabels.map((label) => (
                <Badge key={label} variant="secondary">
                  {label}
                </Badge>
              ))}
            </div>
          ) : (
            <span className="text-muted-foreground">—</span>
          ),
        size: 190,
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
        accessorKey: "accessPathCount",
        id: "accessPathCount",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Zugriffspfade"
            visibility
            column={column}
          />
        ),
        cell: ({ row }) => (
          <Badge variant="outline">
            {row.original.accessPathCount} Pfad
            {row.original.accessPathCount === 1 ? "" : "e"}
          </Badge>
        ),
        size: 140,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
    ],
    [],
  );

  const gridState = useProfileRegistryGridState({
    columns,
    diagnosticIndex,
    profiles,
  });

  const {
    activeFilterCount,
    diagnosticsOnly,
    filteredRows,
    kindCounts,
    originCounts,
    searchQuery,
    selectedKinds,
    selectedOrigins,
    selectedRow,
    setDiagnosticsOnly,
    setSearchQuery,
    setSelectedRow,
    table,
    toggleKind,
    toggleOrigin,
  } = gridState;

  return (
    <>
      <RegistryGridShell
        table={table}
        recordCount={filteredRows.length}
        isLoading={loading}
        loadingMessage="Profile werden geladen…"
        emptyMessage="Keine Registry-Profile gefunden."
        title="Quellenprofile"
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
              label="Profile suchen"
              name="profile-registry-search"
              placeholder="Profile suchen…"
              clearLabel="Profilsuche leeren"
            />
            <ProfileFilterPopover
              selectedKinds={selectedKinds}
              selectedOrigins={selectedOrigins}
              diagnosticsOnly={diagnosticsOnly}
              kindCounts={kindCounts}
              originCounts={originCounts}
              activeFilterCount={activeFilterCount}
              onKindChange={toggleKind}
              onOriginChange={toggleOrigin}
              onDiagnosticsOnlyChange={setDiagnosticsOnly}
            />
            <Button type="button" onClick={onAdd}>
              <PlusIcon data-icon="inline-start" aria-hidden="true" />
              Profil hinzufügen
            </Button>
          </>
        }
        pagination={
          <DataGridPagination
            sizes={[5, 10, 25, 50]}
            rowsPerPageLabel="Zeilen pro Seite"
            info="{from}–{to} von {count}"
            previousPageLabel="Vorherige Profil-Seite"
            nextPageLabel="Nächste Profil-Seite"
          />
        }
        onRowClick={setSelectedRow}
        rowClassName={(row) => registryRowHealthClassName(row.health)}
        actionsClassName="items-center"
      >
        <DataGridScrollArea>
          <DataGridTable />
        </DataGridScrollArea>
      </RegistryGridShell>

      <ProfileDetailsDrawer
        row={selectedRow}
        diagnostics={
          selectedRow
            ? (diagnosticIndex.byProfileKey.get(selectedRow.key) ?? [])
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
