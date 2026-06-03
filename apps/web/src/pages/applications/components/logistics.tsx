"use client"

import * as React from "react"

import { cn } from "@/lib/utils"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"

import { shipments } from "./shipment-data"
import { ShipmentDetails } from "./shipment-details"
import { ShipmentList } from "./shipment-list"

export function Logistics() {
  const [detailsOpen, setDetailsOpen] = React.useState(false)
  const [shipmentListOpen, setShipmentListOpen] = React.useState(true)
  const [selectedShipmentId, setSelectedShipmentId] = React.useState<
    string | null
  >(shipments[0]?.id ?? null)
  const selectedShipment =
    shipments.find((shipment) => shipment.id === selectedShipmentId) ??
    shipments[0] ??
    null

  function handleSelectShipment(shipmentId: string) {
    setSelectedShipmentId(shipmentId)

    if (window.innerWidth < 1024) {
      setDetailsOpen(true)
    }
  }

  function handleToggleShipmentList() {
    if (window.innerWidth < 1024) {
      setDetailsOpen(false)
      return
    }

    setShipmentListOpen((open) => !open)
  }

  return (
    <>
      <div
        data-content-padding="false"
        className={cn(
          "grid h-full overflow-hidden transition-[grid-template-columns] duration-200 ease-linear",
          shipmentListOpen
            ? "lg:grid-cols-[400px_minmax(0,1fr)] lg:divide-x"
            : "lg:grid-cols-[minmax(0,1fr)]"
        )}
      >
        <div
          className={cn(
            "h-full overflow-hidden",
            !shipmentListOpen && "lg:hidden"
          )}
        >
          <ShipmentList
            shipments={shipments}
            selectedShipmentId={selectedShipmentId}
            onSelectShipment={handleSelectShipment}
          />
        </div>
        <div className="hidden h-full overflow-hidden lg:block">
          <ShipmentDetails
            shipment={selectedShipment}
            shipmentListOpen={shipmentListOpen}
            onToggleShipmentList={handleToggleShipmentList}
          />
        </div>
      </div>

      <Sheet open={detailsOpen} onOpenChange={setDetailsOpen}>
        <SheetContent
          side="right"
          className="gap-0 p-0 data-[side=right]:w-full data-[side=right]:sm:max-w-none data-[side=right]:md:w-3/4"
        >
          <SheetHeader className="sr-only">
            <SheetTitle>
              {selectedShipment
                ? `Shipment ${selectedShipment.id}`
                : "Shipment details"}
            </SheetTitle>
            <SheetDescription>
              Selected shipment details and route map.
            </SheetDescription>
          </SheetHeader>
          <ShipmentDetails
            shipment={selectedShipment}
            shipmentListOpen={shipmentListOpen}
            onToggleShipmentList={handleToggleShipmentList}
          />
        </SheetContent>
      </Sheet>
    </>
  )
}
