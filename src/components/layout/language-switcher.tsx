"use client"

import { ChevronDownIcon } from "lucide-react"
import { useTranslation } from "react-i18next"

import { useLocale } from "@/context/locale-provider-context"
import { languageOptions } from "@/lib/i18n/language"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

export function LanguageSwitcher() {
  const { t } = useTranslation()
  const { locale, metadata, setLocale } = useLocale()

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="flex items-center gap-2"
            aria-label={t("language.actions.select")}
          >
            <span aria-hidden="true">{metadata.flag}</span>
            <span>{metadata.label}</span>
            <ChevronDownIcon aria-hidden="true" />
          </Button>
        }
      />
      <DropdownMenuContent align="end">
        {languageOptions.map((language) => (
          <DropdownMenuItem
            key={language.value}
            onClick={() => void setLocale(language.value)}
            className="flex items-center gap-2"
            aria-current={language.value === locale ? "true" : undefined}
          >
            <span aria-hidden="true">{language.flag}</span>
            <span>{language.label}</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
