import type { ColumnDef } from "@tanstack/react-table";

import { Badge } from "@/components/reui/badge";
import { DataGridColumnHeader } from "@/components/reui/data-grid/data-grid-column-header";
import { RegistryStateIndicator } from "@/features/sources/registry/shared/registry-state-indicator";
import type { ProfileGridRow } from "@/features/sources/view-model/profile-grid-model";

export const profileGridColumns: ColumnDef<ProfileGridRow>[] = [
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
      <DataGridColumnHeader
        title="Deklarierter Support"
        visibility
        column={column}
      />
    ),
    cell: ({ row }) => (
      <Badge variant="outline">{row.original.supportLabel}</Badge>
    ),
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
    accessorKey: "detectionEvidenceSummary",
    id: "detectionEvidenceSummary",
    header: ({ column }) => (
      <DataGridColumnHeader
        title="Detection-Evidenz"
        visibility
        column={column}
      />
    ),
    cell: ({ row }) =>
      row.original.detectionEvidenceLabels.length ? (
        <div className="flex min-w-0 flex-wrap gap-1">
          {row.original.detectionEvidenceLabels.map((label) => (
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
      <DataGridColumnHeader title="Zugriffspfade" visibility column={column} />
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
];
