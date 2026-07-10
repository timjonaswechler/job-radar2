"use client";

import * as React from "react";

import { SearchIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import {
  COMMAND_SEARCH_KEYBOARD_SHORTCUT_LABEL,
  useCommandSearch,
} from "@/context/command-search-provider-context";
import type { TranslationKey } from "@/lib/i18n/resources";
import { AppLink } from "@/app/navigation/app-link";
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

function getAvailableItems(items: readonly CommandSearchItem[]) {
  return items.filter((item) => !item.disabled);
}

function groupBy(items: readonly CommandSearchItem[]) {
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
  return t(item.titleKey);
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
  const linkClickSelectionRef = React.useRef<string | null>(null);
  const recommendations = React.useMemo(
    () => getAvailableItems(commandSearchItems),
    [],
  );
  const visibleItems = query ? commandSearchItems : recommendations;

  const handleSelect = (item: CommandSearchItem) => {
    if (item.disabled) return;

    if (linkClickSelectionRef.current === item.id) {
      linkClickSelectionRef.current = null;
      return;
    }

    closeCommandSearch();
    navigateTo(item.url);
  };

  const handleLinkClick = (
    item: CommandSearchItem,
    event: React.MouseEvent<HTMLAnchorElement>,
  ) => {
    linkClickSelectionRef.current = item.id;
    queueMicrotask(() => {
      if (linkClickSelectionRef.current === item.id) {
        linkClickSelectionRef.current = null;
      }
    });

    if (
      event.button === 0 &&
      !event.metaKey &&
      !event.ctrlKey &&
      !event.shiftKey &&
      !event.altKey
    ) {
      closeCommandSearch();
    }
  };

  const renderGroups = (items: readonly CommandSearchItem[]) =>
    groupBy(items).map(({ groupKey, items: groupItems }, index) => (
      <React.Fragment key={groupKey}>
        {index > 0 && <CommandSeparator />}
        <CommandGroup heading={t(groupKey)}>
          {groupItems.map((item) => {
            const label = getItemLabel(item, t);

            const content = (
              <>
                <item.icon aria-hidden="true" />
                <span>{label}</span>
                {item.disabled ? (
                  <Badge variant="outline" className="text-xs">
                    {t("common.status.soon")}
                  </Badge>
                ) : null}
              </>
            );

            if (item.disabled) {
              return (
                <CommandItem disabled key={item.id} value={label}>
                  {content}
                </CommandItem>
              );
            }

            return (
              <CommandItem
                asChild
                key={item.id}
                value={label}
                onSelect={() => handleSelect(item)}
              >
                <AppLink
                  href={item.url}
                  onClick={(event) => handleLinkClick(item, event)}
                >
                  {content}
                </AppLink>
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
