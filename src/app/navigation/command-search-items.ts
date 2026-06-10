import type { LucideIcon } from "lucide-react";

import { sidebarItems } from "@/app/navigation/sidebar/sidebar-items";
import type { TranslationKey } from "@/lib/i18n/resources";

export type CommandSearchItem = {
  id: string;
  groupKey: TranslationKey;
  titleKey: TranslationKey;
  parentTitleKey?: TranslationKey;
  url: string;
  icon?: LucideIcon;
  disabled?: boolean;
  newTab?: boolean;
};

export const commandSearchItems: CommandSearchItem[] = sidebarItems.flatMap(
  (group) =>
    group.items.flatMap((item) => {
      if (item.subItems?.length) {
        return item.subItems.map((subItem) => ({
          id: `page:${subItem.url}`,
          groupKey: "commandSearch.groups.pages",
          titleKey: subItem.titleKey,
          parentTitleKey: item.titleKey,
          url: subItem.url,
          icon: subItem.icon ?? item.icon,
          disabled: subItem.comingSoon,
          newTab: subItem.newTab,
        })) satisfies CommandSearchItem[];
      }

      return [
        {
          id: `page:${item.url}`,
          groupKey: "commandSearch.groups.pages",
          titleKey: item.titleKey,
          url: item.url,
          icon: item.icon,
          disabled: item.comingSoon,
          newTab: item.newTab,
        },
      ] satisfies CommandSearchItem[];
    }),
);
