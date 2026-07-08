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
  baseFontSizePx: {
    storageKey: string
    default: number
    min: number
    max: number
  }
  windowDragRegionEnabled: {
    storageKey: string
    default: boolean
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
  baseFontSizePx: {
    storageKey: "job-radar-base-font-size-px",
    default: 16,
    min: 12,
    max: 24,
  },
  windowDragRegionEnabled: {
    storageKey: "job-radar-window-drag-region-enabled",
    default: true,
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

export function isBaseFontSizePx(value: number): boolean {
  return (
    Number.isInteger(value) &&
    value >= APP_SETTINGS.baseFontSizePx.min &&
    value <= APP_SETTINGS.baseFontSizePx.max
  )
}

export function readStoredWindowDragRegionEnabled(): boolean {
  return (
    window.localStorage.getItem(APP_SETTINGS.windowDragRegionEnabled.storageKey) !==
    "false"
  )
}

export function writeStoredWindowDragRegionEnabled(enabled: boolean) {
  window.localStorage.setItem(
    APP_SETTINGS.windowDragRegionEnabled.storageKey,
    String(enabled)
  )
}
