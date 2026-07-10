import type { CSSProperties, ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";

import { AppHeader } from "./app-header";
import { AppSidebar } from "./app-sidebar";

type AppLayoutProps = {
  pathname: string;
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
  pathname,
  title,
  windowDragRegionEnabled,
  children,
}: AppLayoutProps) {
  const { t } = useTranslation();

  return (
    <SidebarProvider
      className="h-svh overflow-hidden"
      style={sidebarProviderStyle}
    >
      <a
        href="#main-content"
        className="fixed top-2 left-2 z-100 -translate-y-16 rounded-md bg-background px-3 py-2 text-sm font-medium shadow-md transition-transform hover:bg-muted focus:translate-y-0 focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none"
      >
        {t("navigation.skipToMain")}
      </a>
      <WindowTopDragRegion enabled={windowDragRegionEnabled} />
      <AppSidebar
        pathname={pathname}
        variant="inset"
        collapsible="icon"
        windowDragRegionEnabled={windowDragRegionEnabled}
      />
      <SidebarInset
        id="main-content"
        tabIndex={-1}
        className="relative z-20 min-h-0 min-w-0 overflow-hidden focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring focus-visible:outline-none peer-data-[variant=inset]:border [--dashboard-header-height:--spacing(12)]"
      >
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
