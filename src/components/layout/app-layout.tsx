import type { CSSProperties, ReactNode } from "react";

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";

import { AppHeader } from "./app-header";
import { AppSidebar } from "./app-sidebar";

type AppLayoutProps = {
  title: string;
  windowDragRegionEnabled: boolean;
  children: ReactNode;
};

const sidebarProviderStyle = {
  "--sidebar-width": "calc(var(--spacing) * 72)",
  "--header-height": "calc(var(--spacing) * 12)",
  "--window-top-drag-height": "calc(var(--spacing) * 14)",
} as CSSProperties;

type DragRegionProps = {
  "data-tauri-drag-region"?: string;
};

function WindowTopDragRegion({ enabled }: { enabled: boolean }) {
  const dragRegionProps: DragRegionProps = enabled
    ? { "data-tauri-drag-region": "" }
    : {};

  return (
    <div
      className="window-top-drag-region fixed inset-x-0 top-0 h-(--window-top-drag-height) cursor-default select-none"
      aria-hidden="true"
      {...dragRegionProps}
    />
  );
}

export function AppLayout({
  title,
  windowDragRegionEnabled,
  children,
}: AppLayoutProps) {
  return (
    <SidebarProvider
      className="h-svh overflow-hidden"
      style={sidebarProviderStyle}
    >
      <WindowTopDragRegion enabled={windowDragRegionEnabled} />
      <AppSidebar
        variant="inset"
        collapsible="icon"
        windowDragRegionEnabled={windowDragRegionEnabled}
      />
      <SidebarInset className="relative z-20 min-h-0 min-w-0 overflow-hidden peer-data-[variant=inset]:border [--dashboard-header-height:--spacing(12)]">
        <AppHeader
          title={title}
          windowDragRegionEnabled={windowDragRegionEnabled}
        />

        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden p-0">
          {children}
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
