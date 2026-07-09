import { useDeferredValue, useEffect, useMemo, useState } from "react";

import type { PaginationState, SortingState } from "@tanstack/react-table";
import {
  getCoreRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";

import { profileGridColumns } from "@/features/sources/registry/profile/profile-grid-columns";
import type { DiagnosticIndex } from "@/features/sources/view-model/diagnostics";
import {
  countProfileKinds,
  countProfileOrigins,
  createProfileGridRows,
  filterProfileGridRows,
  type ProfileGridRow,
} from "@/features/sources/view-model/profile-grid-model";
import type {
  RegistrySourceProfile,
  SourceProfileKind,
  SourceRegistryDocumentOrigin,
} from "@/lib/api/sources";

type UseProfileRegistryTabOptions = {
  profiles: RegistrySourceProfile[];
  diagnosticIndex: DiagnosticIndex;
};

export function useProfileRegistryTab({
  profiles,
  diagnosticIndex,
}: UseProfileRegistryTabOptions) {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedKinds, setSelectedKinds] = useState<SourceProfileKind[]>([]);
  const [selectedOrigins, setSelectedOrigins] = useState<
    SourceRegistryDocumentOrigin[]
  >([]);
  const [diagnosticsOnly, setDiagnosticsOnly] = useState(false);
  const [selectedRow, setSelectedRow] = useState<ProfileGridRow | null>(null);
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [sorting, setSorting] = useState<SortingState>([
    { id: "name", desc: false },
  ]);
  const [columnOrder, setColumnOrder] = useState<string[]>(() =>
    profileGridColumns.map((column) => column.id as string),
  );
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

  const table = useReactTable({
    columns: profileGridColumns,
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

  const kindCounts = useMemo(() => countProfileKinds(rows), [rows]);
  const originCounts = useMemo(() => countProfileOrigins(rows), [rows]);
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
    closeDetails() {
      setSelectedRow(null);
    },
    toggleKind(kind: SourceProfileKind, checked: boolean) {
      setSelectedKinds((current) => toggleSelectedValue(current, kind, checked));
    },
    toggleOrigin(origin: SourceRegistryDocumentOrigin, checked: boolean) {
      setSelectedOrigins((current) => toggleSelectedValue(current, origin, checked));
    },
  };
}

function toggleSelectedValue<T>(values: T[], value: T, checked: boolean) {
  return checked ? [...values, value] : values.filter((current) => current !== value);
}

export type ProfileRegistryTabState = ReturnType<typeof useProfileRegistryTab>;
