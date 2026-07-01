import { useDeferredValue, useEffect, useMemo, useState } from "react";

import type {
  ColumnDef,
  PaginationState,
  SortingState,
} from "@tanstack/react-table";
import {
  getCoreRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
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
import { ProfileDetailsDrawer } from "@/features/sources/components/registry-details";
import {
  ProfileFilterPopover,
  ProfileRegistryStateDot,
  RegistrySearchInput,
  registryRowHealthClassName,
} from "@/features/sources/components/registry-toolbar";
import {
  countOrigins,
  countProfileKinds,
  createProfileGridRows,
  filterProfileGridRows,
  type DiagnosticIndex,
  type ProfileGridRow,
} from "@/features/sources/registry-view-model";
import type {
  AdapterMetadata,
  RegistrySourceProfile,
  SourceProfileKind,
  SourceRegistryDocumentOrigin,
} from "@/lib/api/sources";

type ProfileRegistryDataGridProps = {
  profiles: RegistrySourceProfile[];
  adaptersByKey: Map<string, AdapterMetadata>;
  diagnosticIndex: DiagnosticIndex;
  loading: boolean;
  onAdd: () => void;
};

export function ProfileRegistryDataGrid({
  profiles,
  adaptersByKey,
  diagnosticIndex,
  loading,
  onAdd,
}: ProfileRegistryDataGridProps) {
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [sorting, setSorting] = useState<SortingState>([
    { id: "name", desc: false },
  ]);
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedKinds, setSelectedKinds] = useState<SourceProfileKind[]>([]);
  const [selectedOrigins, setSelectedOrigins] = useState<
    SourceRegistryDocumentOrigin[]
  >([]);
  const [diagnosticsOnly, setDiagnosticsOnly] = useState(false);
  const [selectedRow, setSelectedRow] = useState<ProfileGridRow | null>(null);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  const rows = useMemo(
    () =>
      createProfileGridRows(
        profiles,
        adaptersByKey,
        diagnosticIndex.byProfileKey,
      ),
    [adaptersByKey, diagnosticIndex.byProfileKey, profiles],
  );

  const filteredRows = useMemo(
    () =>
      filterProfileGridRows(rows, {
        searchQuery: deferredSearchQuery,
        kinds: selectedKinds,
        origins: selectedOrigins,
        diagnosticsOnly,
      }),
    [deferredSearchQuery, diagnosticsOnly, rows, selectedKinds, selectedOrigins],
  );

  const kindCounts = useMemo(() => countProfileKinds(rows), [rows]);
  const originCounts = useMemo(() => countOrigins(rows), [rows]);
  const activeFilterCount =
    selectedKinds.length + selectedOrigins.length + (diagnosticsOnly ? 1 : 0);

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
            <ProfileRegistryStateDot row={row.original} />
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

  const [columnOrder, setColumnOrder] = useState<string[]>(
    columns.map((column) => column.id as string),
  );

  const table = useReactTable({
    columns,
    data: filteredRows,
    pageCount: Math.ceil((filteredRows.length || 0) / pagination.pageSize),
    getRowId: (row) => row.key,
    state: {
      pagination,
      sorting,
      columnOrder,
    },
    columnResizeMode: "onChange",
    onColumnOrderChange: setColumnOrder,
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  useEffect(() => {
    setPagination((current) => ({ ...current, pageIndex: 0 }));
  }, [deferredSearchQuery, diagnosticsOnly, selectedKinds, selectedOrigins]);

  return (
    <>
      <DataGrid
        table={table}
        recordCount={filteredRows.length}
        isLoading={loading}
        loadingMessage="Profile werden geladen…"
        emptyMessage="Keine Registry-Profile gefunden."
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
        <Frame className="w-full" stacked dense>
          <FrameHeader className="gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div className="grid gap-1.5">
              <FrameTitle>Quellenprofile</FrameTitle>
              <FrameDescription>
                Der Punkt vor dem Namen zeigt den Registry-Zustand. Bei
                Problemen Zeile anklicken, um Details im Drawer zu öffnen.
              </FrameDescription>
            </div>
            <div className="flex flex-wrap items-center gap-2.5">
              <RegistrySearchInput
                value={searchQuery}
                onChange={setSearchQuery}
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
                onKindChange={(kind, checked) =>
                  setSelectedKinds((current) =>
                    checked
                      ? [...current, kind]
                      : current.filter((value) => value !== kind),
                  )
                }
                onOriginChange={(origin, checked) =>
                  setSelectedOrigins((current) =>
                    checked
                      ? [...current, origin]
                      : current.filter((value) => value !== origin),
                  )
                }
                onDiagnosticsOnlyChange={setDiagnosticsOnly}
              />
              <Button type="button" onClick={onAdd}>
                <PlusIcon data-icon="inline-start" aria-hidden="true" />
                Profil hinzufügen
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
              previousPageLabel="Vorherige Profil-Seite"
              nextPageLabel="Nächste Profil-Seite"
            />
          </FrameFooter>
        </Frame>
      </DataGrid>

      <ProfileDetailsDrawer
        row={selectedRow}
        adaptersByKey={adaptersByKey}
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
