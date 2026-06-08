import { APP_SETTINGS, isAppTheme, type AppTheme } from "@/lib/app-settings"

export function readStoredTheme(): AppTheme {
  const storedTheme = localStorage.getItem(APP_SETTINGS.theme.storageKey)
  return isAppTheme(storedTheme) ? storedTheme : APP_SETTINGS.theme.default
}

export function writeStoredTheme(theme: AppTheme) {
  localStorage.setItem(APP_SETTINGS.theme.storageKey, theme)
}

export function applyThemeToDocument(theme: AppTheme) {
  const root = document.documentElement

  root.classList.remove("light", "dark")
  root.classList.add(theme)
  root.dataset.theme = theme
  root.style.colorScheme = theme
}

export function toggleTheme(theme: AppTheme): AppTheme {
  return theme === "dark" ? "light" : "dark"
}
