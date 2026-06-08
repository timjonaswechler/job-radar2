import type { SupportedLanguage } from "@/lib/i18n/resources"

export type AppTheme = "light" | "dark"

export type AppSettings = {
  theme: {
    storageKey: string
    default: AppTheme
    values: readonly AppTheme[]
  }
  language: {
    storageKey: string
    default: SupportedLanguage
    values: readonly SupportedLanguage[]
  }
}

export const APP_SETTINGS: AppSettings = {
  theme: {
    storageKey: "job-radar-theme",
    default: "dark",
    values: ["light", "dark"],
  },
  language: {
    storageKey: "job-radar-language",
    default: "de",
    values: ["de", "en"],
  },
}

export function isAppTheme(value: string | null): value is AppTheme {
  return APP_SETTINGS.theme.values.includes(value as AppTheme)
}

export function isAppLanguage(
  value: string | null | undefined
): value is SupportedLanguage {
  return APP_SETTINGS.language.values.includes(value as SupportedLanguage)
}
