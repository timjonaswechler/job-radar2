import type { CSSProperties, ReactNode } from "react";

import { ScrollArea } from "@/components/ui/scroll-area";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";

import { AppHeader } from "./app-header";
import { AppSidebar } from "./app-sidebar";

type AppLayoutProps = {
  title: string;
  children: ReactNode;
};

const sidebarProviderStyle = {
  "--sidebar-width": "calc(var(--spacing) * 72)",
  "--header-height": "calc(var(--spacing) * 12)",
} as CSSProperties;

export function AppLayout({ title, children }: AppLayoutProps) {
  return (
    <SidebarProvider
      className="h-svh overflow-hidden"
      style={sidebarProviderStyle}
    >
      <AppSidebar variant="inset" collapsible="icon" />
      <SidebarInset className="relative z-20 min-h-0 min-w-0 overflow-hidden peer-data-[variant=inset]:border [--dashboard-header-height:--spacing(12)]">
        <AppHeader title={title} />

        <ScrollArea className="min-h-0 flex-1">
          <div className="min-w-0 p-4">{children}</div>
        </ScrollArea>
      </SidebarInset>
    </SidebarProvider>
  );
}
