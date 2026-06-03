export type AppTheme = "light" | "dark"

export type AppSettings = {
  theme: {
    storageKey: string
    default: AppTheme
    values: readonly AppTheme[]
  }
}

export const APP_SETTINGS: AppSettings = {
  theme: {
    storageKey: "job-radar-theme",
    default: "dark",
    values: ["light", "dark"],
  },
}

export function isAppTheme(value: string | null): value is AppTheme {
  return APP_SETTINGS.theme.values.includes(value as AppTheme)
}
