"use client"

import * as React from "react"

import { Search } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui//badge"
import { Button } from "@/components/ui//button"
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui//command"
import type { TranslationKey } from "@/lib/i18n/resources"
import { navigateTo } from "@/navigation/path"
import type { NavMainItem } from "@/navigation/sidebar/sidebar-items"
import { sidebarItems } from "@/navigation/sidebar/sidebar-items"

type SearchItem = {
  group: string
  label: string
  url: string
  icon?: NavMainItem["icon"]
  disabled?: boolean
  newTab?: boolean
}

function createSearchItems(t: (key: TranslationKey) => string): SearchItem[] {
  return sidebarItems.flatMap((group) =>
    group.items.flatMap((item) => {
      const groupLabel = t(group.labelKey ?? "navigation.groups.other")

      if (item.subItems) {
        return item.subItems.map((sub) => ({
          group: groupLabel,
          label: t(sub.titleKey),
          url: sub.url,
          icon: item.icon,
          disabled: sub.comingSoon,
          newTab: sub.newTab,
        }))
      }
      return [
        {
          group: groupLabel,
          label: t(item.titleKey),
          url: item.url,
          icon: item.icon,
          disabled: item.comingSoon,
          newTab: item.newTab,
        },
      ]
    })
  )
}

function getAvailableItems(items: SearchItem[]) {
  return items.filter(
    (item) => !item.disabled && !item.url.includes("coming-soon")
  )
}

function groupBy(items: SearchItem[]) {
  const groups = [...new Set(items.map((item) => item.group))]
  return groups.map((group) => ({
    group,
    items: items.filter((item) => item.group === group),
  }))
}

export function SearchDialog() {
  const { t } = useTranslation()
  const searchItems = React.useMemo(() => createSearchItems(t), [t])
  const recommendations = React.useMemo(
    () => getAvailableItems(searchItems),
    [searchItems]
  )
  const [open, setOpen] = React.useState(false)
  const [query, setQuery] = React.useState("")

  React.useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "j" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        setOpen((prev) => !prev)
      }
    }
    document.addEventListener("keydown", down)
    return () => document.removeEventListener("keydown", down)
  }, [])

  const handleOpenChange = (value: boolean) => {
    setOpen(value)
    if (!value) setQuery("")
  }

  const handleSelect = (item: SearchItem) => {
    if (item.disabled) return
    handleOpenChange(false)
    if (item.newTab) {
      window.open(item.url, "_blank", "noopener,noreferrer")
    } else {
      navigateTo(item.url)
    }
  }

  const renderGroups = (items: SearchItem[]) =>
    groupBy(items).map(({ group, items: groupItems }, index) => (
      <React.Fragment key={group}>
        {index > 0 && <CommandSeparator />}
        <CommandGroup heading={group}>
          {groupItems.map((item) => (
            <CommandItem
              disabled={item.disabled}
              key={`${group}-${item.url}-${item.label}`}
              value={`${item.group} ${item.label}`}
              onSelect={() => handleSelect(item)}
            >
              {item.icon && <item.icon />}
              <span>{item.label}</span>

              {item.disabled && (
                <Badge variant="outline" className="text-xs">
                  {t("search.soon")}
                </Badge>
              )}
            </CommandItem>
          ))}
        </CommandGroup>
      </React.Fragment>
    ))

  return (
    <>
      <Button
        onClick={() => handleOpenChange(true)}
        variant="link"
        className="px-0! font-normal text-muted-foreground hover:no-underline"
      >
        <Search data-icon="inline-start" />
        {t("search.button")}
        <kbd className="inline-flex h-5 items-center gap-1 rounded border bg-muted px-1.5 text-[10px] font-medium select-none">
          <span className="text-xs">⌘</span>J
        </kbd>
      </Button>
      <CommandDialog open={open} onOpenChange={handleOpenChange}>
        <Command>
          <CommandInput
            placeholder={t("search.placeholder")}
            value={query}
            onValueChange={setQuery}
          />
          <CommandList>
            <CommandEmpty>{t("search.noResults")}</CommandEmpty>
            {query ? renderGroups(searchItems) : renderGroups(recommendations)}
          </CommandList>
        </Command>
      </CommandDialog>
    </>
  )
}
