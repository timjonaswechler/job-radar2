import { useDeferredValue, useEffect, useMemo, useState } from "react";

import type { PaginationState, SortingState } from "@tanstack/react-table";
import {
  getCoreRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";

import { sourceGridColumns } from "@/features/sources/registry/source/source-grid-columns";
import type { DiagnosticIndex } from "@/features/sources/view-model/diagnostics";
import {
  countOrigins,
  countSourceStatuses,
  createSourceGridRows,
  filterSourceGridRows,
  type SourceGridRow,
} from "@/features/sources/view-model/source-grid-model";
import type {
  RegistrySource,
  RegistrySourceProfile,
  SourceRegistryDocumentOrigin,
  SourceStatus,
} from "@/lib/api/sources";

type UseSourceRegistryTabOptions = {
  sources: RegistrySource[];
  profilesByKey: Map<string, RegistrySourceProfile>;
  diagnosticIndex: DiagnosticIndex;
};

export function useSourceRegistryTab({
  sources,
  profilesByKey,
  diagnosticIndex,
}: UseSourceRegistryTabOptions) {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedStatuses, setSelectedStatuses] = useState<SourceStatus[]>([]);
  const [selectedOrigins, setSelectedOrigins] = useState<
    SourceRegistryDocumentOrigin[]
  >([]);
  const [diagnosticsOnly, setDiagnosticsOnly] = useState(false);
  const [selectedRow, setSelectedRow] = useState<SourceGridRow | null>(null);
  const [editingSource, setEditingSource] = useState<RegistrySource | null>(null);
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [sorting, setSorting] = useState<SortingState>([
    { id: "name", desc: false },
  ]);
  const [columnOrder, setColumnOrder] = useState<string[]>(() =>
    sourceGridColumns.map((column) => column.id as string),
  );
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

  const table = useReactTable({
    columns: sourceGridColumns,
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
  }, [deferredSearchQuery, diagnosticsOnly, selectedOrigins, selectedStatuses]);

  const statusCounts = useMemo(() => countSourceStatuses(rows), [rows]);
  const originCounts = useMemo(() => countOrigins(rows), [rows]);
  const activeFilterCount =
    selectedStatuses.length +
    selectedOrigins.length +
    (diagnosticsOnly ? 1 : 0);

  return {
    activeFilterCount,
    diagnosticsOnly,
    editingSource,
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
    closeDetails() {
      setSelectedRow(null);
    },
    closeEdit() {
      setEditingSource(null);
    },
    editSource(source: RegistrySource) {
      setSelectedRow(null);
      setEditingSource(source);
    },
    toggleOrigin(origin: SourceRegistryDocumentOrigin, checked: boolean) {
      setSelectedOrigins((current) => toggleSelectedValue(current, origin, checked));
    },
    toggleStatus(status: SourceStatus, checked: boolean) {
      setSelectedStatuses((current) => toggleSelectedValue(current, status, checked));
    },
  };
}

function toggleSelectedValue<T>(values: T[], value: T, checked: boolean) {
  return checked ? [...values, value] : values.filter((current) => current !== value);
}

export type SourceRegistryTabState = ReturnType<typeof useSourceRegistryTab>;
