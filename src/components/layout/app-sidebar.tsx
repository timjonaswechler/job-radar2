"use client";

import type { ComponentProps } from "react";

import { InboxIcon, PlusCircleIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { navigateTo } from "@/app/navigation/path";
import { sidebarItems } from "@/app/navigation/sidebar/sidebar-items";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarFooter,
} from "@/components/ui/sidebar";

import { NavMain } from "@/components/layout/nav-main";
import { ScrollArea } from "@/components/ui/scroll-area";
import { PostingsSidebar } from "@/features/postings/components/postings-sidebar";

export function AppSidebar(props: ComponentProps<typeof Sidebar>) {
  const { t } = useTranslation();
  return (
    <Sidebar {...props}>
      <div className="sidebar-glow-shell flex h-full flex-col">
        <SidebarHeader className="relative z-10 px-2 pt-12 pb-1">
          <SidebarMenu>
            <SidebarMenuItem className="flex items-center gap-2">
              <SidebarMenuButton
                type="button"
                tooltip={t("features.applications.actions.new")}
                className="drop-shadow-jumbo relative z-10 min-w-8 bg-primary text-primary-foreground duration-200 ease-linear hover:bg-primary/90 hover:text-primary-foreground active:bg-primary/90 active:text-primary-foreground"
                onClick={() => navigateTo("/bewerbungen")}
              >
                <PlusCircleIcon />
                <span>{t("features.applications.actions.new")}</span>
              </SidebarMenuButton>
              <Button
                size="icon"
                className="relative z-0 size-9 shrink-0 group-data-[collapsible=icon]:opacity-0"
                variant="outline"
                onClick={() => navigateTo("/postings")}
              >
                <InboxIcon />
                <span className="sr-only">
                  {t("navigation.items.postingsInbox")}
                </span>
              </Button>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>
        <SidebarContent className="relative z-10">
          <ScrollArea className="h-auto">
            <PostingsSidebar />
            <NavMain items={sidebarItems} />
          </ScrollArea>
        </SidebarContent>
        <SidebarFooter>Asdasd</SidebarFooter>
      </div>
    </Sidebar>
  );
}
