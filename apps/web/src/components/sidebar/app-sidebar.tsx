"use client"

import { Command } from "lucide-react"

import { APP_CONFIG } from "@/config/app-config"
import { sidebarItems } from "@/navigation/sidebar/sidebar-items"
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@workspace/ui/components//sidebar"

import { NavMain } from "./nav-main"

export function AppSidebar(props: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              type="button"
              onClick={() => window.location.assign("/")}
            >
              <Command />
              <span className="text-base font-semibold">{APP_CONFIG.name}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={sidebarItems} />
      </SidebarContent>
    </Sidebar>
  )
}
