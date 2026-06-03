import { useState } from "react"
import {
  AlertTriangleIcon,
  PanelLeftIcon,
  Plane,
  Ship,
  Truck,
} from "lucide-react"

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs"
import { cn } from "@/lib/utils"

import type { Shipment } from "./shipment-data"

const modeIcons = {
  air: Plane,
  land: Truck,
  sea: Ship,
} as const

const progressRingClasses: Record<Shipment["status"], string> = {
  Scheduled: "text-muted-foreground",
  "In Transit": "text-primary",
  "Out for Delivery": "text-primary",
  Delivered: "text-green-600",
  Delayed: "text-destructive",
  "On Hold": "text-amber-500",
  "Customs Hold": "text-amber-500",
}

const statusBadgeClasses: Record<Shipment["status"], string> = {
  Scheduled: "border-muted bg-muted/50 text-muted-foreground",
  "In Transit": "border-primary/20 bg-primary/10 text-primary",
  "Out for Delivery": "border-primary/20 bg-primary/10 text-primary",
  Delivered: "border-green-600/20 bg-green-600/10 text-green-600",
  Delayed: "border-destructive/20 bg-destructive/10 text-destructive",
  "On Hold":
    "border-amber-500/20 bg-amber-500/10 text-amber-600 dark:text-amber-400",
  "Customs Hold":
    "border-amber-500/20 bg-amber-500/10 text-amber-600 dark:text-amber-400",
}

type ShipmentDetailsProps = {
  shipment: Shipment | null
  shipmentListOpen?: boolean
  onToggleShipmentList?: () => void
}

type ShipmentDetailsTab =
  | "overview"
  | "research"
  | "coverletter"

function getContactLabel(mode: Shipment["mode"]) {
  if (mode === "land") {
    return "Call Driver"
  }

  if (mode === "air") {
    return "Call Airline Support"
  }

  return "Call Captain"
}

function getTransportNumberLabel(mode: Shipment["mode"]) {
  if (mode === "land") {
    return "Vehicle number"
  }

  if (mode === "air") {
    return "Flight number"
  }

  return "Vessel number"
}

function EmptyShipmentOverview() {
  return (
    <div className="grid min-h-48 place-items-center rounded-lg border border-dashed text-sm text-muted-foreground">
      Select a shipment to view details.
    </div>
  )
}

function ShipmentOverview({ shipment }: { shipment: Shipment }) {
  const ContactIcon = modeIcons[shipment.mode]
  const contactLabel = getContactLabel(shipment.mode)
  const transportNumberLabel = getTransportNumberLabel(shipment.mode)

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        <div className="flex items-center gap-2">
          <h1 className="text-lg font-medium tracking-tight tabular-nums sm:text-xl">
            {shipment.id}
          </h1>
        </div>

        <div className="flex items-center gap-2 text-xs sm:text-sm">
          <Badge
            variant="outline"
            className={cn("gap-1.5", statusBadgeClasses[shipment.status])}
          >
            <span
              className={cn(
                "size-1.5 rounded-full bg-current",
                progressRingClasses[shipment.status]
              )}
            />
            {shipment.status}
          </Badge>
        </div>
      </div>

      {/*<Separator />*/}

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="flex flex-col gap-1">
            <div className="text-sm leading-none font-medium">
              {shipment.customer.name}
            </div>
          </div>
        </div>

        <div className="flex flex-col items-end gap-1">
          <div className="text-xs leading-none text-muted-foreground">
            {shipment.customer.tierLabel}
          </div>
        </div>
      </div>

      <Separator />

      <div className="flex flex-col gap-8">
        <div className="flex items-start justify-between gap-4">
          <h2 className="font-medium">Cargo details</h2>

          <Button variant="outline" size="sm">
            <ContactIcon data-icon="inline-start" />
            {contactLabel}
          </Button>
        </div>

        <div className="grid grid-cols-2 gap-x-4 gap-y-5 md:grid-cols-[1.35fr_1fr_1.1fr_1.15fr_1fr]">
          <div className="col-span-2 flex flex-col gap-1 md:col-span-1 md:gap-2">
            <div className="text-xs leading-none text-muted-foreground md:invisible md:text-sm">
              Cargo
            </div>
            <div className="text-sm leading-none whitespace-nowrap">
              {shipment.cargo}
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <div className="text-xs leading-none text-muted-foreground md:text-sm">
              Total weight
            </div>
            <div className="text-sm leading-none">{shipment.weight}</div>
          </div>

          <div className="flex flex-col gap-2">
            <div className="text-xs leading-none text-muted-foreground md:text-sm">
              Transport mode
            </div>
            <div className="text-sm leading-none capitalize">
              {shipment.mode} · {shipment.routeType}
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <div className="text-xs leading-none text-muted-foreground md:text-sm">
              {transportNumberLabel}
            </div>
            <div className="text-sm leading-none">
              {shipment.transportNumber}
            </div>
          </div>

          <div className="flex flex-col gap-2 md:text-right">
            <div className="text-xs leading-none text-muted-foreground md:text-sm">
              Status
            </div>
            <div className="text-sm leading-none">
              {shipment.progress}% complete
            </div>
          </div>
        </div>
      </div>

      <Separator />

      <Alert className="border-amber-200 bg-amber-50 text-amber-900 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-50">
        <AlertTriangleIcon />
        <AlertTitle>{shipment.handling.label}</AlertTitle>
        <AlertDescription className="space-y-2">
          <div className="border-amber-900 leading-none text-amber-900 dark:border-amber-50 dark:text-amber-50">
            {shipment.handling.note}
          </div>

          <Separator className="bg-amber-800 dark:bg-amber-50" />

          <div className="flex flex-wrap gap-2">
            {shipment.handling.tags.map(({ icon: TagIcon, label }) => (
              <Badge
                className="rounded-sm border-amber-200 bg-background/50 text-amber-900 dark:border-amber-900 dark:text-amber-50"
                key={label}
                variant="outline"
              >
                <TagIcon data-icon="inline-start" />
                {label}
              </Badge>
            ))}
          </div>
        </AlertDescription>
      </Alert>
    </div>
  )
}

function ApplicationResearch({ shipment }: { shipment: Shipment }) {
  if (!shipment) return null

  return <div />
}

function ApplicationsListTrigger({
  className,
  onClick,
  onToggleShipmentList,
  ...props
}: React.ComponentProps<typeof Button> & {
  onToggleShipmentList?: () => void
}) {
  return (
    <Button
      data-sidebar="trigger"
      data-slot="sidebar-trigger"
      variant="ghost"
      size="icon-sm"
      className={cn(className)}
      onClick={(event) => {
        onClick?.(event)
        onToggleShipmentList?.()
      }}
      {...props}
    >
      <PanelLeftIcon />
      <span className="sr-only">Toggle applications list</span>
    </Button>
  )
}

export function ShipmentDetails({
  shipment,
  shipmentListOpen,
  onToggleShipmentList,
}: ShipmentDetailsProps) {
  const [activeTab, setActiveTab] = useState<ShipmentDetailsTab>("overview")

  if (!shipment) {
    return (
      <div className="grid h-full min-h-0 grid-rows-[320px_1fr] overflow-hidden lg:grid-rows-[420px_1fr]">
        <div className="min-h-0 overflow-hidden p-4">
          <EmptyShipmentOverview />
        </div>
      </div>
    )
  }

  return (
    <div className="grid h-full min-h-0 grid-rows-[1fr] overflow-hidden">
      <div className="min-h-0 overflow-hidden">
        <div className="h-full min-h-0 py-4">

          <Tabs
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as ShipmentDetailsTab)}
            className="h-full gap-0"
          >

            <TabsList
              className="w-full justify-start gap-2 border-b px-4 **:data-[slot=tabs-trigger]:text-xs sm:gap-4 sm:**:data-[slot=tabs-trigger]:text-sm"
              variant="line"
            >
              <ApplicationsListTrigger
                aria-label={
                  shipmentListOpen
                    ? "Hide applications list"
                    : "Show applications list"
                }
                aria-pressed={shipmentListOpen}
                onToggleShipmentList={onToggleShipmentList}
              />
              <TabsTrigger className="flex-none" value="overview">
                Overview
              </TabsTrigger>
              <TabsTrigger className="flex-none" value="research">
                Research
              </TabsTrigger>
              <TabsTrigger className="flex-none" value="coverletter">
                Cover letter
              </TabsTrigger>
            </TabsList>
            <TabsContent className="min-h-0 overflow-auto p-4" value="overview">
              <ShipmentOverview shipment={shipment} />
            </TabsContent>
            <TabsContent
              className="min-h-0 overflow-hidden p-4"
              value="application"
            >

            </TabsContent>
            <TabsContent
              className="min-h-0 overflow-hidden p-4"
              value="research"
            >
              <ApplicationResearch shipment={shipment} />
            </TabsContent>
            <TabsContent
              className="min-h-0 overflow-hidden p-4"
              value="coverletter"
            />
          </Tabs>
        </div>
      </div>
    </div>
  )
}
