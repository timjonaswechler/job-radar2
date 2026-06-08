"use client"

import { useMemo, useState } from "react"
import { Badge } from "@/components/reui/badge"
import { DataGrid } from "@/components/reui/data-grid/data-grid"
import { DataGridColumnHeader } from "@/components/reui/data-grid/data-grid-column-header"
import { DataGridPagination } from "@/components/reui/data-grid/data-grid-pagination"
import { DataGridScrollArea } from "@/components/reui/data-grid/data-grid-scroll-area"
import { DataGridTable } from "@/components/reui/data-grid/data-grid-table"
import {
  ColumnDef,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  PaginationState,
  SortingState,
  useReactTable,
} from "@tanstack/react-table"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardFooter,
  CardHeader,
  CardTitle,
  CardDescription
} from "@/components/ui/card"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Kbd } from "@/components/ui/kbd"
import { Search ,Plus } from 'lucide-react'

interface IData {
  id: string
  source: string
  status: "active" | "inactive"
  url: string
  company: string
  role: string
  joined: string
  category: string
  balance: number
}

const demoData: IData[] = [
  {
    id: "1",
    source: "Schott AG",
    status: "active",
    url: "https://example.com",
    company: "Apple",
    role: "CEO",
    joined: "Jan, 2024",
    category: "SAP SuccessFactors",
    balance: 5143.03,
  },
  {
    id: "2",
    source: "StepStone",
    status: "inactive",
    url: "https://example.com",
    company: "OpenAI",
    role: "CTO",
    joined: "Mar, 2023",
    category: "Playwright Scraper",
    balance: 4321.87,
  },
  {
    id: "3",
    source: "Michael Rodriguez",
    status: "active",
    url: "https://example.com",
    company: "Meta",
    role: "Designer",
    joined: "Jun, 2022",
    category: "Technology",
    balance: 7654.98,
  },
  {
    id: "4",
    source: "Emma Wilson",
    status: "inactive",
    url: "https://example.com",
    company: "Tesla",
    role: "Developer",
    joined: "Sep, 2024",
    category: "Technology",
    balance: 3456.45,
  },
  {
    id: "5",
    source: "David Kim",
    status: "active",
    url: "https://example.com",
    company: "SAP",
    role: "Lawyer",
    joined: "Nov, 2023",
    category: "Business",
    balance: 9876.54,
  },
  {
    id: "6",
    source: "Aron Thompson",
    status: "active",
    url: "https://example.com",
    company: "Keenthemes",
    role: "Director",
    joined: "Feb, 2022",
    category: "Technology",
    balance: 6214.22,
  },
  {
    id: "7",
    source: "James Brown",
    status: "inactive",
    url: "https://example.com",
    company: "BBVA",
    role: "Product Manager",
    joined: "Aug, 2024",
    category: "Business",
    balance: 5321.77,
  },
  {
    id: "8",
    source: "Maria Garcia",
    status: "active",
    url: "https://example.com",
    company: "Sony",
    role: "Marketing Lead",
    joined: "Dec, 2023",
    category: "Technology",
    balance: 8452.39,
  },
  {
    id: "9",
    source: "Nick Johnson",
    status: "active",
    url: "https://example.com",
    company: "LVMH",
    role: "Data Scientist",
    joined: "Apr, 2022",
    category: "Business",
    balance: 7345.1,
  },
]

export function Pattern() {
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  })
  const [sorting, setSorting] = useState<SortingState>([
    { id: "status", desc: false },
  ])

  const columns = useMemo<ColumnDef<IData>[]>(
    () => [
      {
        accessorKey: "source",
        id: "source",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Source"
            visibility={true}
            column={column}
          />
        ),
        cell: ({ row }) => {
          return (
            <div className="flex items-center gap-3">
              <div className="space-y-px">
                <div className={cn("font-medium", { "text-foreground/30": row.original.status === "inactive" }, { "text-foreground": row.original.status === "active" })}>
                  {row.original.source}
                </div>
                <div className={cn({ "text-muted-foreground/30": row.original.status === "inactive" }, { "text-muted-foreground": row.original.status === "active" })}>
                  {row.original.url}
                </div>
              </div>
            </div>
          )
        },
        size: 300,
        enableSorting: true,
        enableHiding: false,
        enableResizing: true,
      },
      {
        accessorKey: "category",
        id: "category",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Category"
            visibility={true}
            column={column}
          />
        ),
        cell: ({ row }) => {
          return (
            <div className="flex items-center gap-1.5">
              <div className={cn("font-medium", { "text-foreground/30": row.original.status === "inactive" }, { "text-foreground": row.original.status === "active" })}>
                {row.original.category}
              </div>
            </div>
          )
        },
        size: 250,
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
        cell: ({ row }) => {
          const status = row.original.status

          if (status == "active") {
            return <Badge variant="success-light">Active</Badge>
          } else {
            return <Badge variant="destructive-light">Inactive</Badge>
          }
        },
        size: 200,
        enableSorting: true,
        enableHiding: true,
        enableResizing: false,
      },
      {
        accessorKey: "flag",
        id: "flag",
        header: ({ column }) => (
          <DataGridColumnHeader
            title="Action"
            visibility={true}
            column={column}
          />
        ),
        cell: ({ row }) => {
          return (
            <div>

            </div>
          )
        },
        size: 200,
        enableSorting: false,
        enableHiding: true,
        enableResizing: false,
      },
    ],
    []
  )

  const [columnOrder, setColumnOrder] = useState<string[]>(
    columns.map((column) => column.id as string)
  )



  const table = useReactTable({
    columns,
    data: demoData,
    pageCount: Math.ceil((demoData?.length || 0) / pagination.pageSize),
    getRowId: (row: IData) => row.id,
    state: {
      pagination,
      sorting,
      columnOrder,
    },
    columnResizeMode: "onChange",
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
    onColumnOrderChange: setColumnOrder,
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getSortedRowModel: getSortedRowModel(),
  })

  const searchQuery =
    (table.getColumn("search")?.getFilterValue() as string) ?? ""

  return (
    <DataGrid
      table={table}
      recordCount={demoData?.length || 0}
      tableClassNames={{
        edgeCell: "px-5",
      }}
      tableLayout={{
        columnsPinnable: true,
        columnsResizable: true,
        columnsMovable: true,
        columnsVisibility: true,
      }}
    >
      <Card className="w-full gap-3 py-3.5">
        <CardHeader className="has-data-[slot=card-action]:grid-cols-1 md:has-data-[slot=card-action]:grid-cols-[1fr_auto]">
          <CardTitle className="text-xl leading-none">Job Sources</CardTitle>
          <CardDescription className="max-w-sm leading-snug">
            Manage your sources where the job listings are fetched from.
          </CardDescription>
          <CardAction className="col-start-1 row-start-auto flex w-full flex-wrap justify-start gap-2 justify-self-stretch md:col-start-2 md:row-span-2 md:row-start-1 md:w-auto md:flex-nowrap md:justify-end md:justify-self-end">
            <InputGroup className="h-7 w-full md:w-64">
              <InputGroupAddon align="inline-start">
                <Search className="size-3.5" />
              </InputGroupAddon>
              <InputGroupInput
                className="h-7"
                placeholder="Search users..."
                value={searchQuery}
                onChange={(event) => {
                  table
                    .getColumn("search")
                    ?.setFilterValue(event.target.value || undefined)
                  table.setPageIndex(0)
                }}
              />
              <InputGroupAddon align="inline-end">
                <Kbd className="h-4 text-[10px]">⌘K</Kbd>
              </InputGroupAddon>
            </InputGroup>
            <Button size="sm">
              <Plus /> Add Job Source
            </Button>
          </CardAction>
        </CardHeader>
        <div className="w-full border-y">
          <DataGridScrollArea>
            <DataGridTable />
          </DataGridScrollArea>
        </div>
        <CardFooter className="border-none bg-transparent! px-3.5 py-0">
          <DataGridPagination />
        </CardFooter>
      </Card>
    </DataGrid>
  )
}
