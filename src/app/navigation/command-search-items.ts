import type { LucideIcon } from "lucide-react";

import {
  navigationManifest,
  type NavigationId,
} from "@/app/navigation/navigation-manifest";
import type { TranslationKey } from "@/lib/i18n/resources";

export type CommandSearchItem = {
  id: string;
  navigationId: NavigationId;
  groupKey: TranslationKey;
  titleKey: TranslationKey;
  url: string;
  icon: LucideIcon;
  disabled?: boolean;
};

export const commandSearchItems: readonly CommandSearchItem[] =
  navigationManifest.flatMap((item) =>
    item.commandSearch
      ? [
          {
            id: `page:${item.id}`,
            navigationId: item.id,
            groupKey: item.commandSearch.groupKey,
            titleKey: item.titleKey,
            url: item.path,
            icon: item.icon,
            disabled:
              "comingSoon" in item ? Boolean(item.comingSoon) : undefined,
          },
        ]
      : [],
  );
