"use client"

import { useEffect, useMemo, useState, type CSSProperties } from "react"
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard"
import { Badge } from "@/components/reui/badge"
import { DataGrid } from "@/components/reui/data-grid/data-grid"
import { DataGridColumnHeader } from "@/components/reui/data-grid/data-grid-column-header"
import { DataGridPagination } from "@/components/reui/data-grid/data-grid-pagination"
import { DataGridScrollArea } from "@/components/reui/data-grid/data-grid-scroll-area"
import {
  DataGridTable,
  DataGridTableRowSelect,
  DataGridTableRowSelectAll,
} from "@/components/reui/data-grid/data-grid-table"
import {
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type PaginationState,
  type Row,
  type SortingState,
} from "@tanstack/react-table"
import {
  CopyIcon,
  ExternalLinkIcon,
  FunnelIcon,
  MoreHorizontalIcon,
  PlusIcon,
  SearchIcon,
  XIcon,
} from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardFooter,
  CardHeader,
} from "@/components/ui/card"
import { Checkbox } from "@/components/ui/checkbox"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group"
import { Label } from "@/components/ui/label"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { useLocale } from "@/context/locale-provider-context"
import { getDashboardPostings } from "@/lib/api/dashboard"
import { cn } from "@/lib/utils"

import type { DashboardPosting, PostingStatus, WorkModel } from "../types"

const postingStatuses = [
  "new",
  "interesting",
  "review_later",
  "hidden",
  "converted_to_application",
] as const satisfies readonly PostingStatus[]

const postingStatusLabels: Record<PostingStatus, string> = {
  new: "Neu",
  interesting: "Interessant",
  review_later: "Später ansehen",
  hidden: "Ausgeblendet",
  converted_to_application: "In Bewerbung",
}

const workModelLabels: Record<WorkModel, string> = {
  remote: "Remote",
  hybrid: "Hybrid",
  on_site: "Vor Ort",
  unknown: "Unbekannt",
}

const progressRingClasses: Record<PostingStatus, string> = {
  new: "text-muted-foreground",
  interesting: "text-primary",
  review_later: "text-amber-500",
  hidden: "text-destructive",
  converted_to_application: "text-green-600",
}

const statusBadgeClasses: Record<PostingStatus, string> = {
  new: "border-muted bg-muted/50 text-muted-foreground",
  interesting: "border-primary/20 bg-primary/10 text-primary",
  review_later:
    "border-amber-500/20 bg-amber-500/10 text-amber-600 dark:text-amber-400",
  hidden: "border-destructive/20 bg-destructive/10 text-destructive",
  converted_to_application:
    "border-green-600/20 bg-green-600/10 text-green-600",
}

const statusRingAngles: Record<PostingStatus, number> = {
  new: 90,
  interesting: 220,
  review_later: 140,
  hidden: 360,
  converted_to_application: 360,
}

type StatusRingStyle = CSSProperties & {
  "--angle": string
}

function createEmptyStatusCounts() {
  return postingStatuses.reduce(
    (acc, status) => {
      acc[status] = 0
      return acc
    },
    {} as Record<PostingStatus, number>
  )
}

function getProgressRingClass(status: PostingStatus) {
  return cn(
    "grid size-3 place-items-center rounded-full bg-[conic-gradient(currentColor_0deg_var(--angle),transparent_var(--angle)_360deg)] p-[0.5px]",
    progressRingClasses[status]
  )
}

function StatusBadge({ status }: { status: PostingStatus }) {
  const angle = statusRingAngles[status]

  return (
    <Badge
      variant="outline"
      className={cn("gap-1.5", statusBadgeClasses[status])}
    >
      <span
        style={{ "--angle": `${angle}deg` } as StatusRingStyle}
        className={getProgressRingClass(status)}
      >
        <span className="grid size-2 place-items-center rounded-full bg-card">
          <span className="size-1 rounded-full bg-current" />
        </span>
      </span>
      {postingStatusLabels[status]}
    </Badge>
  )
}

function getCompanyInitials(company: string) {
  return (
    company
      .trim()
      .split(/\s+/)
      .map((part) => part[0])
      .join("")
      .slice(0, 2)
      .toUpperCase() || "?"
  )
}

function getLocationLabel(posting: DashboardPosting) {
  if (posting.primaryLocation && posting.region) {
    return `${posting.primaryLocation} · ${posting.region}`
  }

  return posting.primaryLocation ?? posting.region ?? "Ort unbekannt"
}

function getFindingLabel(posting: DashboardPosting) {
  if (posting.latestSourceName) {
    return posting.findingCount > 1
      ? `${posting.latestSourceName} +${posting.findingCount - 1}`
      : posting.latestSourceName
  }

  if (posting.findingCount === 1) return "1 Fundstelle"
  if (posting.findingCount > 1) return `${posting.findingCount} Fundstellen`

  return "Manuell erfasst"
}

function ActionsCell({ row }: { row: Row<DashboardPosting> }) {
  const { copyToClipboard } = useCopyToClipboard()
  const latestResultUrl = row.original.latestResultUrl

  const handleCopyId = () => {
    copyToClipboard(row.original.id)

    toast.success("Stellenanzeigen-ID kopiert", {
      description: row.original.id,
    })
  }

  const handleCopyLink = () => {
    if (!latestResultUrl) return

    copyToClipboard(latestResultUrl)

    toast.success("Fundstellen-Link kopiert", {
      description: latestResultUrl,
    })
  }

  const handleOpenLink = () => {
    if (!latestResultUrl) return

    window.open(latestResultUrl, "_blank", "noopener,noreferrer")
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button className="size-7" size="icon" variant="ghost">
            <MoreHorizontalIcon />
          </Button>
        }
      />
      <DropdownMenuContent side="bottom" align="start">
        <DropdownMenuItem disabled={!latestResultUrl} onClick={handleOpenLink}>
          <ExternalLinkIcon />
          Quelle öffnen
        </DropdownMenuItem>
        <DropdownMenuItem disabled={!latestResultUrl} onClick={handleCopyLink}>
          <CopyIcon />
          Link kopieren
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={handleCopyId}>ID kopieren</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

export function RecentPostingsTable() {
  "use no memo"
  // TanStack Table v8 returns a mutable table instance; keep this component
  // out of React Compiler memoization to avoid stale table UI.
  const { formatDateTime } = useLocale()
  const [postings, setPostings] = useState<DashboardPosting[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 5,
  })
  const [sorting, setSorting] = useState<SortingState>([
    { id: "lastFoundAt", desc: true },
  ])
  const [searchQuery, setSearchQuery] = useState("")
  const [selectedStatuses, setSelectedStatuses] = useState<PostingStatus[]>([])

  useEffect(() => {
    let cancelled = false

    setIsLoading(true)

    getDashboardPostings()
      .then((postings) => {
        if (cancelled) return
        toast.dismiss("dashboard-postings-load-error")
        setPostings(postings)
      })
      .catch((error: unknown) => {
        if (cancelled) return
        toast.error("Stellenanzeigen konnten nicht geladen werden", {
          id: "dashboard-postings-load-error",
          description: String(error),
        })
      })
      .finally(() => {
        if (cancelled) return
        setIsLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [])

  const filteredPostings = useMemo(() => {
    return postings.filter((posting) => {
      const matchesStatus =
        selectedStatuses.length === 0 ||
        selectedStatuses.includes(posting.status)

      const searchLower = searchQuery.toLowerCase()
      const matchesSearch =
        !searchQuery ||
        Object.values(posting).join(" ").toLowerCase().includes(searchLower)

      return matchesStatus && matchesSearch
    })
  }, [postings, searchQuery, selectedStatuses])

  const statusCounts = useMemo(() => {
    return postings.reduce((acc, posting) => {
      acc[posting.status] = (acc[posting.status] || 0) + 1
      return acc
    }, createEmptyStatusCounts())
  }, [postings])

  const availableStatuses = useMemo(() => {
    return postingStatuses.filter((status) => statusCounts[status] > 0)
  }, [statusCounts])

  const handleStatusChange = (checked: boolean, value: PostingStatus) => {
    setPagination((prev) => ({ ...prev, pageIndex: 0 }))
    setSelectedStatuses((prev) =>
      checked ? [...prev, value] : prev.filter((status) => status !== value)
    )
  }

  const handleSearchChange = (value: string) => {
    setPagination((prev) => ({ ...prev, pageIndex: 0 }))
    setSearchQuery(value)
  }

  const columns = useMemo<ColumnDef<DashboardPosting>[]>(
    () => [
      {
        accessorKey: "id",
        id: "id",
        header: () => <DataGridTableRowSelectAll />,
        cell: ({ row }) => <DataGridTableRowSelect row={row} />,
        enableSorting: false,
        size: 35,
        meta: {
          headerClassName: "",
          cellClassName: "",
        },
        enableResizing: false,
      },
      {
        accessorKey: "title",
        id: "title",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Stellenanzeige"
            visibility={true}
            column={column}
          />
        ),
        cell: ({ row }) => {
          return (
            <div className="flex items-center gap-3">
              <div className="grid size-9 shrink-0 place-items-center rounded-md bg-muted text-xs font-medium text-muted-foreground">
                {getCompanyInitials(row.original.company)}
              </div>
              <div className="min-w-0 space-y-px">
                <div className="truncate font-medium text-foreground">
                  {row.original.title}
                </div>
                <div className="truncate text-muted-foreground">
                  {row.original.company}
                </div>
              </div>
            </div>
          )
        },
        size: 260,
        enableSorting: true,
        enableHiding: false,
        enableResizing: true,
      },
      {
        accessorFn: getLocationLabel,
        id: "location",
        header: ({ column }) => (
          <DataGridColumnHeader title="Ort" visibility={true} column={column} />
        ),
        cell: ({ row }) => {
          return (
            <div className="truncate font-medium text-foreground">
              {getLocationLabel(row.original)}
            </div>
          )
        },
        size: 170,
        meta: {
          headerClassName: "",
          cellClassName: "text-start",
        },
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorKey: "workModel",
        id: "workModel",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Arbeitsmodell"
            visibility={true}
            column={column}
          />
        ),
        cell: ({ row }) => {
          return (
            <div className="font-medium text-foreground">
              {workModelLabels[row.original.workModel]}
            </div>
          )
        },
        size: 130,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorKey: "descriptionExcerpt",
        id: "descriptionExcerpt",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Beschreibung"
            visibility={true}
            column={column}
          />
        ),
        cell: ({ row }) => {
          return (
            <div className="line-clamp-2 text-muted-foreground">
              {row.original.descriptionExcerpt ||
                "Keine Beschreibung gespeichert"}
            </div>
          )
        },
        size: 340,
        enableSorting: false,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorFn: (posting) => posting.lastFoundAt ?? posting.createdAt,
        id: "lastFoundAt",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Gefunden"
            visibility={true}
            column={column}
          />
        ),
        cell: ({ row }) => {
          const foundAt = row.original.lastFoundAt ?? row.original.createdAt

          return (
            <div className="space-y-px">
              <div className="font-medium text-foreground">
                {formatDateTime(foundAt, {
                  dateStyle: "medium",
                  timeStyle: "short",
                })}
              </div>
              <div className="truncate text-muted-foreground">
                {getFindingLabel(row.original)}
              </div>
            </div>
          )
        },
        size: 180,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        accessorKey: "status",
        id: "status",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Status"
            visibility={true}
            column={column}
          />
        ),
        cell: ({ row }) => <StatusBadge status={row.original.status} />,
        size: 160,
        enableSorting: true,
        enableHiding: true,
        enableResizing: true,
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => <ActionsCell row={row} />,
        size: 60,
        enableSorting: false,
        enableHiding: false,
        enableResizing: false,
      },
    ],
    [formatDateTime]
  )

  const [columnOrder, setColumnOrder] = useState<string[]>(
    columns.map((column) => column.id as string)
  )

  // TanStack Table v8 is intentionally incompatible with React Compiler memoization.
  // eslint-disable-next-line react-hooks/incompatible-library
  const table = useReactTable({
    columns,
    data: filteredPostings,
    pageCount: Math.ceil(
      (filteredPostings.length || 0) / pagination.pageSize
    ),
    getRowId: (row: DashboardPosting) => row.id,
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
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getSortedRowModel: getSortedRowModel(),
  })

  return (
    <DataGrid
      table={table}
      recordCount={filteredPostings.length}
      isLoading={isLoading}
      loadingMode="spinner"
      loadingMessage="Stellenanzeigen werden geladen…"
      emptyMessage="Keine gefundenen Stellenanzeigen vorhanden."
      tableLayout={{
        columnsPinnable: true,
        columnsResizable: true,
        columnsMovable: true,
        columnsVisibility: true,
      }}
    >
      <Card className="w-full gap-3 py-0">
        <CardHeader className="flex items-center justify-between px-3.5 py-2">
          <div className="flex items-center gap-2.5">
            <InputGroup className="w-64">
              <InputGroupAddon align="inline-start">
                <SearchIcon />
              </InputGroupAddon>

              <InputGroupInput
                placeholder="Stellenanzeigen suchen…"
                value={searchQuery}
                onChange={(event) => handleSearchChange(event.target.value)}
              />

              {searchQuery.length > 0 && (
                <InputGroupAddon align="inline-end">
                  <InputGroupButton
                    aria-label="Suche zurücksetzen"
                    title="Suche zurücksetzen"
                    size="icon-xs"
                    onClick={() => handleSearchChange("")}
                  >
                    <XIcon />
                  </InputGroupButton>
                </InputGroupAddon>
              )}
            </InputGroup>
            <Popover>
              <PopoverTrigger
                render={
                  <Button variant="outline">
                    <FunnelIcon />
                    Status
                    {selectedStatuses.length > 0 && (
                      <Badge size="sm" variant="info-outline">
                        {selectedStatuses.length}
                      </Badge>
                    )}
                  </Button>
                }
              />
              <PopoverContent className="w-52" align="start">
                <div className="space-y-3">
                  <div className="text-xs font-medium text-muted-foreground">
                    Filter
                  </div>
                  <div className="space-y-3">
                    {availableStatuses.length === 0 ? (
                      <div className="text-xs text-muted-foreground">
                        Noch keine Status vorhanden.
                      </div>
                    ) : (
                      availableStatuses.map((status) => (
                        <div
                          key={status}
                          className="flex items-center gap-2.5"
                        >
                          <Checkbox
                            id={`status-${status}`}
                            checked={selectedStatuses.includes(status)}
                            onCheckedChange={(checked) =>
                              handleStatusChange(checked === true, status)
                            }
                          />
                          <Label
                            htmlFor={`status-${status}`}
                            className="flex grow items-center justify-between gap-1.5 font-normal"
                          >
                            {postingStatusLabels[status]}
                            <span className="text-muted-foreground">
                              {statusCounts[status]}
                            </span>
                          </Label>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              </PopoverContent>
            </Popover>
          </div>
          <CardAction>
            <Button>
              <PlusIcon />
              Manuell hinzufügen
            </Button>
          </CardAction>
        </CardHeader>
        <CardContent className="border-y px-0">
          <DataGridScrollArea>
            <DataGridTable />
          </DataGridScrollArea>
        </CardContent>
        <CardFooter className="border-none bg-transparent! px-3.5 py-2">
          <DataGridPagination />
        </CardFooter>
      </Card>
    </DataGrid>
  )
}
