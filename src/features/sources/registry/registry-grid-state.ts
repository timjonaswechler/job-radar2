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

import {
  countOrigins,
  countProfileKinds,
  countSourceStatuses,
  createProfileGridRows,
  createSourceGridRows,
  filterProfileGridRows,
  filterSourceGridRows,
  type DiagnosticIndex,
  type ProfileGridRow,
  type SourceGridRow,
} from "@/features/sources/view-model/registry-view-model";
import type {
  RegistrySource,
  RegistrySourceProfile,
  SourceProfileKind,
  SourceRegistryDocumentOrigin,
  SourceStatus,
} from "@/lib/api/sources";

type RegistryGridRow = { key: string };

type UseRegistryTableStateOptions<TRow extends RegistryGridRow> = {
  columns: ColumnDef<TRow>[];
  rows: TRow[];
  resetPageDependencies: unknown[];
};

function useRegistryTableState<TRow extends RegistryGridRow>({
  columns,
  rows,
  resetPageDependencies,
}: UseRegistryTableStateOptions<TRow>) {
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [sorting, setSorting] = useState<SortingState>([
    { id: "name", desc: false },
  ]);
  const [columnOrder, setColumnOrder] = useState<string[]>(() =>
    columns.map((column) => column.id as string),
  );

  const table = useReactTable({
    columns,
    data: rows,
    pageCount: Math.ceil((rows.length || 0) / pagination.pageSize),
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
    // resetPageDependencies intentionally owns this effect's invalidation surface.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, resetPageDependencies);

  return table;
}

function toggleSelectedValue<T>(values: T[], value: T, checked: boolean) {
  return checked ? [...values, value] : values.filter((current) => current !== value);
}

type UseSourceRegistryGridStateOptions = {
  columns: ColumnDef<SourceGridRow>[];
  diagnosticIndex: DiagnosticIndex;
  profilesByKey: Map<string, RegistrySourceProfile>;
  sources: RegistrySource[];
};

export function useSourceRegistryGridState({
  columns,
  diagnosticIndex,
  profilesByKey,
  sources,
}: UseSourceRegistryGridStateOptions) {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedStatuses, setSelectedStatuses] = useState<SourceStatus[]>([]);
  const [selectedOrigins, setSelectedOrigins] = useState<
    SourceRegistryDocumentOrigin[]
  >([]);
  const [diagnosticsOnly, setDiagnosticsOnly] = useState(false);
  const [selectedRow, setSelectedRow] = useState<SourceGridRow | null>(null);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  const rows = useMemo(
    () =>
      createSourceGridRows(
        sources,
        profilesByKey,
        diagnosticIndex.bySourceKey,
      ),
    [diagnosticIndex.bySourceKey, profilesByKey, sources],
  );

  const filteredRows = useMemo(
    () =>
      filterSourceGridRows(rows, {
        searchQuery: deferredSearchQuery,
        statuses: selectedStatuses,
        origins: selectedOrigins,
        diagnosticsOnly,
      }),
    [
      deferredSearchQuery,
      diagnosticsOnly,
      rows,
      selectedOrigins,
      selectedStatuses,
    ],
  );

  const table = useRegistryTableState({
    columns,
    rows: filteredRows,
    resetPageDependencies: [
      deferredSearchQuery,
      diagnosticsOnly,
      selectedOrigins,
      selectedStatuses,
    ],
  });

  const statusCounts = useMemo(() => countSourceStatuses(rows), [rows]);
  const originCounts = useMemo(() => countOrigins(rows), [rows]);
  const activeFilterCount =
    selectedStatuses.length +
    selectedOrigins.length +
    (diagnosticsOnly ? 1 : 0);

  return {
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
    toggleOrigin(origin: SourceRegistryDocumentOrigin, checked: boolean) {
      setSelectedOrigins((current) => toggleSelectedValue(current, origin, checked));
    },
    toggleStatus(status: SourceStatus, checked: boolean) {
      setSelectedStatuses((current) => toggleSelectedValue(current, status, checked));
    },
  };
}

type UseProfileRegistryGridStateOptions = {
  columns: ColumnDef<ProfileGridRow>[];
  diagnosticIndex: DiagnosticIndex;
  profiles: RegistrySourceProfile[];
};

export function useProfileRegistryGridState({
  columns,
  diagnosticIndex,
  profiles,
}: UseProfileRegistryGridStateOptions) {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedKinds, setSelectedKinds] = useState<SourceProfileKind[]>([]);
  const [selectedOrigins, setSelectedOrigins] = useState<
    SourceRegistryDocumentOrigin[]
  >([]);
  const [diagnosticsOnly, setDiagnosticsOnly] = useState(false);
  const [selectedRow, setSelectedRow] = useState<ProfileGridRow | null>(null);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  const rows = useMemo(
    () => createProfileGridRows(profiles, diagnosticIndex.byProfileKey),
    [diagnosticIndex.byProfileKey, profiles],
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

  const table = useRegistryTableState({
    columns,
    rows: filteredRows,
    resetPageDependencies: [
      deferredSearchQuery,
      diagnosticsOnly,
      selectedKinds,
      selectedOrigins,
    ],
  });

  const kindCounts = useMemo(() => countProfileKinds(rows), [rows]);
  const originCounts = useMemo(() => countOrigins(rows), [rows]);
  const activeFilterCount =
    selectedKinds.length + selectedOrigins.length + (diagnosticsOnly ? 1 : 0);

  return {
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
    toggleKind(kind: SourceProfileKind, checked: boolean) {
      setSelectedKinds((current) => toggleSelectedValue(current, kind, checked));
    },
    toggleOrigin(origin: SourceRegistryDocumentOrigin, checked: boolean) {
      setSelectedOrigins((current) => toggleSelectedValue(current, origin, checked));
    },
  };
}
