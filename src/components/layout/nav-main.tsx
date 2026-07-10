"use client";

import { useTranslation } from "react-i18next";

import { AppLink } from "@/app/navigation/app-link";
import type {
  NavigationManifestItem,
  SidebarNavigationGroup,
} from "@/app/navigation/navigation-types";
import { isAppPathActive } from "@/app/navigation/path";
import { Badge } from "@/components/ui/badge";
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";

interface NavMainProps {
  readonly items: readonly SidebarNavigationGroup[];
  readonly pathname: string;
}

function NavigationItem({
  item,
  pathname,
}: {
  item: NavigationManifestItem;
  pathname: string;
}) {
  const { t } = useTranslation();
  const label = t(item.titleKey);
  const isActive = isAppPathActive(pathname, item.path);
  const content = (
    <>
      <item.icon aria-hidden="true" />
      <span>{label}</span>
      {item.comingSoon ? (
        <Badge variant="secondary" className="ml-auto text-xs">
          {t("common.status.soon")}
        </Badge>
      ) : null}
    </>
  );

  if (item.comingSoon) {
    return (
      <SidebarMenuButton type="button" tooltip={label} disabled>
        {content}
      </SidebarMenuButton>
    );
  }

  return (
    <SidebarMenuButton
      render={
        <AppLink
          href={item.path}
          aria-current={isActive ? "page" : undefined}
        />
      }
      tooltip={label}
      isActive={isActive}
    >
      {content}
    </SidebarMenuButton>
  );
}

export function NavMain({ items, pathname }: NavMainProps) {
  const { t } = useTranslation();

  return (
    <>
      {items.map((group) => (
        <SidebarGroup key={group.id}>
          {group.labelKey ? (
            <SidebarGroupLabel>{t(group.labelKey)}</SidebarGroupLabel>
          ) : null}
          <SidebarGroupContent className="flex flex-col gap-2">
            <SidebarMenu>
              {group.items.map((item) => (
                <SidebarMenuItem key={item.id}>
                  <NavigationItem item={item} pathname={pathname} />
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      ))}
    </>
  );
}
