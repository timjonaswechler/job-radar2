"use client"

import { useTranslation } from "react-i18next"

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
} from "@/components/ui//sidebar"
import { navigateTo } from "@/navigation/path"
import type { NavGroup, NavMainItem } from "@/navigation/sidebar/sidebar-items"

interface NavMainProps {
  readonly items: readonly NavGroup[]
}

const IsComingSoon = ({ label }: { label: string }) => (
  <span className="ml-auto rounded-md bg-gray-200 px-2 py-1 text-xs dark:text-gray-800">
    {label}
  </span>
)

function goTo(item: NavMainItem) {
  if (item.comingSoon) return

  if (item.newTab) {
    window.open(item.url, "_blank", "noopener,noreferrer")
    return
  }

  navigateTo(item.url)
}

export function NavMain({ items }: NavMainProps) {
  const { t } = useTranslation()
  const path = window.location.pathname

  const isItemActive = (url: string, subItems?: NavMainItem["subItems"]) => {
    if (subItems?.length) {
      return subItems.some((sub) => path.startsWith(sub.url))
    }
    return path === url
  }

  return (
    <>
      {items.map((group) => (
        <SidebarGroup key={group.id}>
          {group.labelKey && (
            <SidebarGroupLabel>{t(group.labelKey)}</SidebarGroupLabel>
          )}
          <SidebarGroupContent className="flex flex-col gap-2">
            <SidebarMenu>
              {group.items.map((item) => (
                <SidebarMenuItem key={item.url}>
                  <SidebarMenuButton
                    type="button"
                    aria-disabled={item.comingSoon}
                    disabled={item.comingSoon}
                    tooltip={t(item.titleKey)}
                    isActive={isItemActive(item.url, item.subItems)}
                    onClick={() => goTo(item)}
                  >
                    {item.icon && <item.icon />}
                    <span>{t(item.titleKey)}</span>
                    {item.comingSoon && <IsComingSoon label={t("search.soon")} />}
                  </SidebarMenuButton>

                  {item.subItems && (
                    <SidebarMenuSub>
                      {item.subItems.map((subItem) => (
                        <SidebarMenuSubItem key={subItem.url}>
                          <SidebarMenuSubButton
                            href={subItem.url}
                            target={subItem.newTab ? "_blank" : undefined}
                            aria-disabled={subItem.comingSoon}
                            isActive={path === subItem.url}
                          >
                            {subItem.icon && <subItem.icon />}
                            <span>{t(subItem.titleKey)}</span>
                            {subItem.comingSoon && (
                              <IsComingSoon label={t("search.soon")} />
                            )}
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
