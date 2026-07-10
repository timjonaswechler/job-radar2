import type { ComponentType } from "react";
import type { LucideIcon } from "lucide-react";

import type { TranslationKey } from "@/lib/i18n/resources";

export type NavigationManifestItem = {
  id: string;
  path: string;
  titleKey: TranslationKey;
  icon: LucideIcon;
  Component: ComponentType;
  sidebar?: {
    groupId: string;
    groupLabelKey?: TranslationKey;
  };
  commandSearch?: {
    groupKey: TranslationKey;
  };
  comingSoon?: boolean;
};

export type SidebarNavigationGroup = {
  id: string;
  labelKey?: TranslationKey;
  items: readonly NavigationManifestItem[];
};
