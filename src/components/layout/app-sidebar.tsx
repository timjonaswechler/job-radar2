"use client";

import type { ComponentProps } from "react";

import { InboxIcon, PlusCircleIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AppLink } from "@/app/navigation/app-link";
import {
  getNavigationItem,
  sidebarNavigationGroups,
} from "@/app/navigation/navigation-manifest";
import { NavMain } from "@/components/layout/nav-main";
import { buttonVariants } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { PostingsSidebar } from "@/features/postings/queues/postings-sidebar";

type AppSidebarProps = ComponentProps<typeof Sidebar> & {
  pathname: string;
  windowDragRegionEnabled?: boolean;
};

type DragRegionProps = {
  "data-tauri-drag-region"?: string;
};

const postingsNavigationItem = getNavigationItem("postings");

export function AppSidebar({
  pathname,
  windowDragRegionEnabled = false,
  ...props
}: AppSidebarProps) {
  const { t } = useTranslation();
  const dragRegionProps: DragRegionProps = windowDragRegionEnabled
    ? { "data-tauri-drag-region": "" }
    : {};
  const newApplicationLabel = t("features.applications.actions.new");

  return (
    <Sidebar {...props}>
      <div className="sidebar-glow-shell flex h-full flex-col">
        <div
          className="app-sidebar-window-drag-region absolute inset-x-0 top-0 z-0 cursor-default select-none"
          aria-hidden="true"
          {...dragRegionProps}
        />
        <SidebarHeader className="app-sidebar-header relative z-10">
          <SidebarMenu>
            <SidebarMenuItem className="flex items-center gap-2">
              <SidebarMenuButton
                type="button"
                tooltip={`${newApplicationLabel} · ${t("common.status.soon")}`}
                className="drop-shadow-jumbo relative z-10 min-w-8 bg-primary text-primary-foreground duration-200 ease-linear hover:bg-primary/90 hover:text-primary-foreground active:bg-primary/90 active:text-primary-foreground"
                disabled
              >
                <PlusCircleIcon aria-hidden="true" />
                <span>{newApplicationLabel}</span>
                <span className="sr-only">({t("common.status.soon")})</span>
              </SidebarMenuButton>
              <AppLink
                href={postingsNavigationItem.path}
                aria-label={t("navigation.items.postingsInbox")}
                className={buttonVariants({
                  size: "icon",
                  variant: "outline",
                  className:
                    "relative z-0 size-9 shrink-0 group-data-[collapsible=icon]:opacity-0",
                })}
              >
                <InboxIcon aria-hidden="true" />
              </AppLink>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>
        <SidebarContent className="relative z-10">
          <nav
            aria-label={t("navigation.sidebarLabel")}
            className="min-h-0 flex-1"
          >
            <ScrollArea className="h-full">
              <PostingsSidebar pathname={pathname} />
              <NavMain items={sidebarNavigationGroups} pathname={pathname} />
            </ScrollArea>
          </nav>
        </SidebarContent>
      </div>
    </Sidebar>
  );
}
