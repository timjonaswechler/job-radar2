import { useDeferredValue, useMemo, useState, type ReactNode } from "react";

import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type Row,
  type SortingState,
} from "@tanstack/react-table";
import { FunnelIcon, PlusIcon, SearchIcon, XIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Frame, FrameDescription, FrameHeader, FramePanel, FrameTitle } from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { searchRequestColumns } from "@/features/search-requests/components/search-requests-table/columns";
import {
  countAttentionRows,
  countSearchRequestStatuses,
  filterSearchRequestRows,
  type SearchRequestGroupId,
  type SearchRequestTableRow,
} from "@/features/search-requests/model/search-request-row-model";
import {
  searchRequestStatusLabels,
  searchRequestStatusOptions,
} from "@/features/search-requests/status";
import type { SearchRequestStatus } from "@/lib/api/search-requests";

type SearchRequestsTableProps = {
  rows: SearchRequestTableRow[];
  onCreate: () => void;
  onRun: (row: SearchRequestTableRow) => void;
  onEdit: (row: SearchRequestTableRow) => void;
  onDelete: (row: SearchRequestTableRow) => void;
  runningRequestId?: number | null;
};

export function SearchRequestsTable({
  rows,
  onCreate,
  onRun,
  onEdit,
  onDelete,
  runningRequestId = null,
}: SearchRequestsTableProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedStatuses, setSelectedStatuses] = useState<SearchRequestStatus[]>([]);
  const [attentionOnly, setAttentionOnly] = useState(false);
  const [sorting, setSorting] = useState<SortingState>([
    { id: "groupRank", desc: false },
    { id: "id", desc: false },
  ]);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  const filteredRows = useMemo(
    () =>
      filterSearchRequestRows(rows, {
        searchQuery: deferredSearchQuery,
        statuses: selectedStatuses,
        attentionOnly,
      }),
    [attentionOnly, deferredSearchQuery, rows, selectedStatuses],
  );
  const columns = useMemo(
    () => searchRequestColumns({ onRun, onEdit, onDelete, runningRequestId }),
    [onRun, onDelete, onEdit, runningRequestId],
  );
  const table = useReactTable({
    columns,
    data: filteredRows,
    state: {
      sorting,
      columnVisibility: { groupRank: false, id: false },
    },
    getRowId: (row) => String(row.id),
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });
  const groupedRows = groupTableRows(table.getRowModel().rows);
  const statusCounts = useMemo(() => countSearchRequestStatuses(rows), [rows]);
  const attentionCount = useMemo(() => countAttentionRows(rows), [rows]);
  const activeFilterCount = selectedStatuses.length + (attentionOnly ? 1 : 0);

  return (
    <Frame className="px-1 w-full" stacked dense>
      <FrameHeader className="gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="grid gap-1.5">
          <FrameTitle>Search Requests</FrameTitle>
          <FrameDescription>
            Gespeicherte Suchintents mit Include-/Exclude-Regeln, Source Keys und Backend-Validierung.
          </FrameDescription>
        </div>
        <div className="flex flex-wrap items-baseline gap-2.5">
          <SearchRequestsSearchInput
            value={searchQuery}
            onChange={setSearchQuery}
          />
          <SearchRequestsFilterPopover
            selectedStatuses={selectedStatuses}
            attentionOnly={attentionOnly}
            statusCounts={statusCounts}
            attentionCount={attentionCount}
            activeFilterCount={activeFilterCount}
            onStatusChange={(status, checked) =>
              setSelectedStatuses((current) =>
                checked
                  ? [...current, status]
                  : current.filter((value) => value !== status),
              )
            }
            onAttentionOnlyChange={setAttentionOnly}
          />
          <Button type="button" onClick={onCreate}>
            <PlusIcon data-icon="inline-start" aria-hidden="true" />
            Search Request erstellen
          </Button>
        </div>
      </FrameHeader>
      <FramePanel className="p-0 shadow-none">
        {groupedRows.length ? (
          <Table>
            <TableHeader>
              {table.getHeaderGroups().map((headerGroup) => (
                <TableRow key={headerGroup.id}>
                  {headerGroup.headers.map((header) => (
                    <TableHead key={header.id}>
                      {header.isPlaceholder ? null : header.column.getCanSort() ? (
                        <Button
                          type="button"
                          variant="ghost"
                          className="-ml-2"
                          onClick={header.column.getToggleSortingHandler()}
                        >
                          {flexRender(header.column.columnDef.header, header.getContext())}
                        </Button>
                      ) : (
                        flexRender(header.column.columnDef.header, header.getContext())
                      )}
                    </TableHead>
                  ))}
                </TableRow>
              ))}
            </TableHeader>
            <TableBody>
              {groupedRows.map((group) => (
                <TableGroup
                  key={group.id}
                  label={group.label}
                  count={group.rows.length}
                  colSpan={table.getVisibleLeafColumns().length}
                >
                  {group.rows.map((row) => (
                    <TableRow
                      key={row.id}
                      className={row.original.groupId === "attention" ? "bg-warning/5 hover:bg-warning/10" : undefined}
                    >
                      {row.getVisibleCells().map((cell) => (
                        <TableCell key={cell.id}>
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </TableCell>
                      ))}
                    </TableRow>
                  ))}
                </TableGroup>
              ))}
            </TableBody>
          </Table>
        ) : (
          <Empty className="border-0">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <SearchIcon aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>
                {rows.length ? "Keine Treffer" : "Noch keine Search Requests"}
              </EmptyTitle>
              <EmptyDescription>
                {rows.length
                  ? "Passe Suche oder Filter an."
                  : "Erstelle die erste Search Request und wähle Sources aus der Registry."}
              </EmptyDescription>
            </EmptyHeader>
            {!rows.length ? (
              <Button type="button" onClick={onCreate}>
                <PlusIcon data-icon="inline-start" aria-hidden="true" />
                Search Request erstellen
              </Button>
            ) : null}
          </Empty>
        )}
      </FramePanel>
    </Frame>
  );
}

function groupTableRows(rows: Array<Row<SearchRequestTableRow>>) {
  const groupOrder: SearchRequestGroupId[] = [
    "attention",
    "active",
    "draft",
    "disabled",
  ];

  return groupOrder.flatMap((groupId) => {
    const groupRows = rows.filter((row) => row.original.groupId === groupId);
    if (!groupRows.length) return [];
    return [
      {
        id: groupId,
        label: groupRows[0]?.original.groupLabel ?? groupId,
        rows: groupRows,
      },
    ];
  });
}

type TableGroupProps = {
  label: string;
  count: number;
  colSpan: number;
  children: ReactNode;
};

function TableGroup({ label, count, colSpan, children }: TableGroupProps) {
  return (
    <>
      <TableRow className="bg-muted/50 hover:bg-muted/50">
        <TableCell colSpan={colSpan} className="font-medium text-muted-foreground">
          {label} · {count}
        </TableCell>
      </TableRow>
      {children}
    </>
  );
}

type SearchRequestsSearchInputProps = {
  value: string;
  onChange: (value: string) => void;
};

function SearchRequestsSearchInput({ value, onChange }: SearchRequestsSearchInputProps) {
  return (
    <InputGroup className="w-64 bg-background">
      <InputGroupAddon align="inline-start">
        <SearchIcon aria-hidden="true" />
      </InputGroupAddon>
      <InputGroupInput
        placeholder="Search Requests suchen…"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {value ? (
        <InputGroupAddon align="inline-end">
          <InputGroupButton
            aria-label="Search-Request-Suche leeren"
            title="Search-Request-Suche leeren"
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

type SearchRequestsFilterPopoverProps = {
  selectedStatuses: SearchRequestStatus[];
  attentionOnly: boolean;
  statusCounts: Record<SearchRequestStatus, number>;
  attentionCount: number;
  activeFilterCount: number;
  onStatusChange: (status: SearchRequestStatus, checked: boolean) => void;
  onAttentionOnlyChange: (checked: boolean) => void;
};

function SearchRequestsFilterPopover({
  selectedStatuses,
  attentionOnly,
  statusCounts,
  attentionCount,
  activeFilterCount,
  onStatusChange,
  onAttentionOnlyChange,
}: SearchRequestsFilterPopoverProps) {
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
            {searchRequestStatusOptions.map(({ value }) => (
              <CheckboxFilterRow
                key={value}
                id={`search-request-status-${value}`}
                label={searchRequestStatusLabels[value]}
                count={statusCounts[value] ?? 0}
                checked={selectedStatuses.includes(value)}
                onCheckedChange={(checked) => onStatusChange(value, checked)}
              />
            ))}
          </FilterGroup>
          <FilterGroup title="Aufmerksamkeit">
            <CheckboxFilterRow
              id="search-request-attention-only"
              label="Nur mit Handlungsbedarf"
              count={attentionCount}
              checked={attentionOnly}
              onCheckedChange={onAttentionOnlyChange}
            />
          </FilterGroup>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function FilterGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="grid gap-2">
      <div className="text-xs font-medium text-muted-foreground">{title}</div>
      <div className="grid gap-2">{children}</div>
    </div>
  );
}

function CheckboxFilterRow({
  id,
  label,
  checked,
  count,
  onCheckedChange,
}: {
  id: string;
  label: string;
  checked: boolean;
  count: number;
  onCheckedChange: (checked: boolean) => void;
}) {
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
        <span className="text-muted-foreground">{count}</span>
      </Label>
    </div>
  );
}
