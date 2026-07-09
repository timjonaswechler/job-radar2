import type { ColumnDef } from "@tanstack/react-table";

import { Badge } from "@/components/reui/badge";
import { DataGridColumnHeader } from "@/components/reui/data-grid/data-grid-column-header";
import { RegistryStateIndicator } from "@/features/sources/registry/shared/registry-state-indicator";
import {
  sourceStatusBadgeVariants,
  validationStateBadgeVariants,
} from "@/features/sources/status";
import type { SourceGridRow } from "@/features/sources/view-model/source-grid-model";

export const sourceGridColumns: ColumnDef<SourceGridRow>[] = [
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
    cell: ({ row }) => <Badge variant="secondary">{row.original.originLabel}</Badge>,
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
];
