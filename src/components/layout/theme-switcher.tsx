"use client"

import { Moon, Sun } from "lucide-react"
import { useTranslation } from "react-i18next"

import { useTheme } from "@/context/theme-provider-context"
import { Button } from "@/components/ui/button"

export function ThemeSwitcher() {
  const { theme, toggleTheme } = useTheme()
  const { t } = useTranslation()
  const label =
    theme === "dark"
      ? t("theme.actions.switchToLight")
      : t("theme.actions.switchToDark")

  return (
    <Button size="icon" onClick={toggleTheme} aria-label={label}>
      {theme === "dark" ? <Sun /> : <Moon />}
    </Button>
  )
}
