import type { CSSProperties, ReactNode } from "react"

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"

import { AppHeader } from "./app-header"
import { AppSidebar } from "./app-sidebar"

type AppLayoutProps = {
  title: string
  children: ReactNode
}

const sidebarProviderStyle = {
  "--sidebar-width": "calc(var(--spacing) * 72)",
  "--header-height": "calc(var(--spacing) * 12)",
} as CSSProperties

export function AppLayout({ title, children }: AppLayoutProps) {
  return (
    <SidebarProvider style={sidebarProviderStyle}>
      <AppSidebar variant="inset" collapsible="icon" />
      <SidebarInset className="relative z-20 min-w-0 peer-data-[variant=inset]:border [--dashboard-header-height:--spacing(12)]">
        <AppHeader title={title} />
        <main className="h-full min-w-0 p-4 md:p-4">{children}</main>
      </SidebarInset>
    </SidebarProvider>
  )
}
