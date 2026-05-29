"use client"

import { Languages } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { SupportedLanguage } from "@/lib/i18n/resources"
import { Button } from "@workspace/ui/components/button"

function getCurrentLanguage(language: string | undefined): SupportedLanguage {
  return language?.startsWith("en") ? "en" : "de"
}

export function LanguageSwitcher() {
  const { i18n, t } = useTranslation()
  const currentLanguage = getCurrentLanguage(i18n.resolvedLanguage)
  const nextLanguage = currentLanguage === "de" ? "en" : "de"
  const label =
    nextLanguage === "de"
      ? t("language.switchToGerman")
      : t("language.switchToEnglish")

  return (
    <Button
      type="button"
      onClick={() => void i18n.changeLanguage(nextLanguage)}
      aria-label={label}
    >
      <Languages data-icon="inline-start" />
      {nextLanguage.toUpperCase()}
    </Button>
  )
}
