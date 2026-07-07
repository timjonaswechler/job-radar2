import type { ColumnDef } from "@tanstack/react-table";
import { MoreHorizontalIcon, PencilIcon, PlayIcon, Trash2Icon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  searchRequestStatusBadgeVariants,
  searchRunStatusBadgeVariants,
} from "@/features/search-requests/status";
import type { SearchRequestTableRow } from "@/features/search-requests/model/search-request-row-model";

type SearchRequestsTableActions = {
  onRun: (row: SearchRequestTableRow) => void;
  onEdit: (row: SearchRequestTableRow) => void;
  onDelete: (row: SearchRequestTableRow) => void;
  runningRequestId?: number | null;
};

export function searchRequestColumns({
  onRun,
  onEdit,
  onDelete,
  runningRequestId = null,
}: SearchRequestsTableActions): ColumnDef<SearchRequestTableRow>[] {
  return [
    {
      accessorKey: "groupRank",
      id: "groupRank",
      enableSorting: true,
    },
    {
      accessorKey: "id",
      id: "id",
      enableSorting: true,
    },
    {
      accessorKey: "title",
      id: "title",
      header: "Search Request",
      cell: ({ row }) => (
        <div className="grid min-w-48 gap-1">
          <span className="font-medium">{row.original.title}</span>
          <span className="truncate text-muted-foreground">
            {row.original.includeSummary}
          </span>
        </div>
      ),
      enableSorting: true,
    },
    {
      accessorKey: "statusLabel",
      id: "statusLabel",
      header: "Status",
      cell: ({ row }) => (
        <Badge variant={searchRequestStatusBadgeVariants[row.original.status]}>
          {row.original.statusLabel}
        </Badge>
      ),
      enableSorting: true,
    },
    {
      accessorKey: "includeSummary",
      id: "rules",
      header: "Regeln",
      cell: ({ row }) => (
        <div className="grid max-w-64 gap-1">
          <span className="truncate">
            Include ({row.original.includeCount}): {row.original.includeSummary}
          </span>
          <span className="truncate text-muted-foreground">
            Exclude ({row.original.excludeCount}): {row.original.excludeSummary}
          </span>
        </div>
      ),
      enableSorting: true,
    },
    {
      accessorKey: "sourceSummary",
      id: "sources",
      header: "Sources",
      cell: ({ row }) => (
        <div className="grid max-w-52 gap-1">
          <span className="truncate">{row.original.sourceSummary}</span>
          <span className="text-muted-foreground">
            {row.original.sourceCount} Source{row.original.sourceCount === 1 ? "" : "s"}
          </span>
          {row.original.missingSourceKeys.length ? (
            <Badge variant="destructive-light">
              {row.original.missingSourceKeys.length} fehlt
            </Badge>
          ) : null}
        </div>
      ),
      enableSorting: true,
    },
    {
      accessorKey: "locationsSummary",
      id: "locations",
      header: "Orte",
      cell: ({ row }) => (
        <div className="grid max-w-48 gap-1">
          <span className="truncate">{row.original.locationsSummary}</span>
          <span className="text-muted-foreground">{row.original.radiusLabel}</span>
        </div>
      ),
      enableSorting: true,
    },
    {
      accessorKey: "validationLabel",
      id: "validation",
      header: "Validierung",
      cell: ({ row }) =>
        row.original.validationError ? (
          <div className="grid max-w-56 gap-1">
            <Badge variant="warning-light">Validation Error</Badge>
            <span className="truncate text-muted-foreground">
              {row.original.validationError}
            </span>
          </div>
        ) : (
          <Badge variant="success-light">OK</Badge>
        ),
      enableSorting: true,
    },
    {
      accessorKey: "lastRunLabel",
      id: "lastRun",
      header: "Letzter Lauf",
      cell: ({ row }) => (
        <div className="grid max-w-56 gap-1">
          {row.original.request.lastRunStatus ? (
            <Badge
              variant={searchRunStatusBadgeVariants[row.original.request.lastRunStatus]}
            >
              {row.original.lastRunLabel.split(" · ")[0]}
            </Badge>
          ) : null}
          <span className="truncate text-muted-foreground">
            {row.original.lastRunLabel}
          </span>
          {row.original.lastRunError ? (
            <span className="truncate text-destructive">
              {row.original.lastRunError}
            </span>
          ) : null}
        </div>
      ),
      enableSorting: true,
    },
    {
      id: "actions",
      header: "Aktionen",
      cell: ({ row }) => {
        const runDisabledReason = getRunDisabledReason(
          row.original,
          runningRequestId,
        );

        return (
          <DropdownMenu>
            <DropdownMenuTrigger
              render={
                <Button type="button" variant="ghost" size="icon-sm">
                  <MoreHorizontalIcon aria-hidden="true" />
                  <span className="sr-only">Aktionen öffnen</span>
                </Button>
              }
            />
            <DropdownMenuContent align="end">
              <DropdownMenuGroup>
                <DropdownMenuItem
                  disabled={runDisabledReason !== null}
                  title={runDisabledReason ?? "Search Request ausführen"}
                  onClick={() => onRun(row.original)}
                >
                  <PlayIcon aria-hidden="true" />
                  Ausführen
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem onClick={() => onEdit(row.original)}>
                  <PencilIcon aria-hidden="true" />
                  Bearbeiten
                </DropdownMenuItem>
                <DropdownMenuItem
                  variant="destructive"
                  onClick={() => onDelete(row.original)}
                >
                  <Trash2Icon aria-hidden="true" />
                  Löschen
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        );
      },
      enableSorting: false,
    },
  ];
}

function getRunDisabledReason(
  row: SearchRequestTableRow,
  runningRequestId: number | null,
) {
  if (runningRequestId === row.id) {
    return "Dieser Search Request läuft bereits.";
  }
  if (runningRequestId !== null) {
    return "Es läuft bereits ein anderer Search Request.";
  }
  if (row.status !== "active") {
    return "Nur aktive Search Requests können ausgeführt werden.";
  }
  if (row.validationError) {
    return "Search Requests mit Validierungsfehlern können nicht ausgeführt werden.";
  }
  if (row.missingSourceKeys.length) {
    return "Search Requests mit fehlenden Source Keys können nicht ausgeführt werden.";
  }
  if (!row.includeCount) {
    return "Search Requests brauchen mindestens eine Include-Regel.";
  }
  if (!row.sourceCount) {
    return "Search Requests brauchen mindestens eine Source.";
  }
  return null;
}
