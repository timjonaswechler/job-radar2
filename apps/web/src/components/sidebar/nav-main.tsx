"use client"

import { MailIcon, PlusCircleIcon } from "lucide-react"

import { Button } from "@workspace/ui/components//button"
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from "@workspace/ui/components//sidebar"
import type { NavGroup, NavMainItem } from "@/navigation/sidebar/sidebar-items"

interface NavMainProps {
  readonly items: readonly NavGroup[]
}

const IsComingSoon = () => (
  <span className="ml-auto rounded-md bg-gray-200 px-2 py-1 text-xs dark:text-gray-800">
    Soon
  </span>
)

function goTo(item: NavMainItem) {
  if (item.comingSoon) return

  if (item.newTab) {
    window.open(item.url, "_blank", "noopener,noreferrer")
    return
  }

  window.location.assign(item.url)
}

export function NavMain({ items }: NavMainProps) {
  const path = window.location.pathname

  const isItemActive = (url: string, subItems?: NavMainItem["subItems"]) => {
    if (subItems?.length) {
      return subItems.some((sub) => path.startsWith(sub.url))
    }
    return path === url
  }

  return (
    <>
      <SidebarGroup>
        <SidebarGroupContent className="flex flex-col gap-2">
          <SidebarMenu>
            <SidebarMenuItem className="flex items-center gap-2">
              <SidebarMenuButton
                type="button"
                tooltip="Quick Create"
                className="min-w-8 bg-primary text-primary-foreground duration-200 ease-linear hover:bg-primary/90 hover:text-primary-foreground active:bg-primary/90 active:text-primary-foreground"
              >
                <PlusCircleIcon />
                <span>Quick Create</span>
              </SidebarMenuButton>
              <Button
                size="icon"
                className="h-9 w-9 shrink-0 group-data-[collapsible=icon]:opacity-0"
                variant="outline"
              >
                <MailIcon />
                <span className="sr-only">Inbox</span>
              </Button>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>

      {items.map((group) => (
        <SidebarGroup key={group.id}>
          {group.label && <SidebarGroupLabel>{group.label}</SidebarGroupLabel>}
          <SidebarGroupContent className="flex flex-col gap-2">
            <SidebarMenu>
              {group.items.map((item) => (
                <SidebarMenuItem key={item.title}>
                  <SidebarMenuButton
                    type="button"
                    aria-disabled={item.comingSoon}
                    disabled={item.comingSoon}
                    tooltip={item.title}
                    isActive={isItemActive(item.url, item.subItems)}
                    onClick={() => goTo(item)}
                  >
                    {item.icon && <item.icon />}
                    <span>{item.title}</span>
                    {item.comingSoon && <IsComingSoon />}
                  </SidebarMenuButton>

                  {item.subItems && (
                    <SidebarMenuSub>
                      {item.subItems.map((subItem) => (
                        <SidebarMenuSubItem key={subItem.title}>
                          <SidebarMenuSubButton
                            href={subItem.url}
                            target={subItem.newTab ? "_blank" : undefined}
                            aria-disabled={subItem.comingSoon}
                            isActive={path === subItem.url}
                          >
                            {subItem.icon && <subItem.icon />}
                            <span>{subItem.title}</span>
                            {subItem.comingSoon && <IsComingSoon />}
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>
                      ))}
                    </SidebarMenuSub>
                  )}
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      ))}
    </>
  )
}
