"use client";

import * as React from "react";

import { SearchIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import {
  COMMAND_SEARCH_KEYBOARD_SHORTCUT_LABEL,
  useCommandSearch,
} from "@/context/command-search-provider-context";
import type { TranslationKey } from "@/lib/i18n/resources";
import { navigateTo } from "@/app/navigation/path";
import {
  commandSearchItems,
  type CommandSearchItem,
} from "@/app/navigation/command-search-items";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";

function getAvailableItems(items: CommandSearchItem[]) {
  return items.filter((item) => !item.disabled);
}

function groupBy(items: CommandSearchItem[]) {
  const groups = [...new Set(items.map((item) => item.groupKey))];
  return groups.map((groupKey) => ({
    groupKey,
    items: items.filter((item) => item.groupKey === groupKey),
  }));
}

function getItemLabel(
  item: CommandSearchItem,
  t: (key: TranslationKey) => string,
) {
  const label = t(item.titleKey);
  return item.parentTitleKey ? `${t(item.parentTitleKey)} › ${label}` : label;
}

export function CommandSearchDialog() {
  const { t } = useTranslation();
  const {
    open,
    query,
    setOpen,
    setQuery,
    openCommandSearch,
    closeCommandSearch,
  } = useCommandSearch();
  const recommendations = React.useMemo(
    () => getAvailableItems(commandSearchItems),
    [],
  );
  const visibleItems = query ? commandSearchItems : recommendations;

  const handleSelect = (item: CommandSearchItem) => {
    if (item.disabled) return;

    closeCommandSearch();

    if (item.newTab) {
      window.open(item.url, "_blank", "noopener,noreferrer");
      return;
    }

    navigateTo(item.url);
  };

  const renderGroups = (items: CommandSearchItem[]) =>
    groupBy(items).map(({ groupKey, items: groupItems }, index) => (
      <React.Fragment key={groupKey}>
        {index > 0 && <CommandSeparator />}
        <CommandGroup heading={t(groupKey)}>
          {groupItems.map((item) => {
            const label = getItemLabel(item, t);

            return (
              <CommandItem
                disabled={item.disabled}
                key={item.id}
                value={label}
                onSelect={() => handleSelect(item)}
              >
                {item.icon && <item.icon />}
                <span>{label}</span>

                {item.disabled && (
                  <Badge variant="outline" className="text-xs">
                    {t("common.status.soon")}
                  </Badge>
                )}
              </CommandItem>
            );
          })}
        </CommandGroup>
      </React.Fragment>
    ));

  return (
    <>
      <Button
        onClick={openCommandSearch}
        variant="link"
        className="px-0! font-normal text-muted-foreground hover:no-underline"
      >
        <SearchIcon data-icon="inline-start" />
        {t("common.actions.search")}
        <kbd className="inline-flex h-5 items-center gap-1 rounded border bg-muted px-1.5 text-[10px] font-medium select-none">
          {COMMAND_SEARCH_KEYBOARD_SHORTCUT_LABEL}
        </kbd>
      </Button>
      <CommandDialog open={open} onOpenChange={setOpen}>
        <Command>
          <CommandInput
            placeholder={t("commandSearch.input.placeholder")}
            value={query}
            onValueChange={setQuery}
          />
          <CommandList>
            <CommandEmpty>{t("common.empty.noResults")}</CommandEmpty>
            {renderGroups(visibleItems)}
          </CommandList>
        </Command>
      </CommandDialog>
    </>
  );
}
