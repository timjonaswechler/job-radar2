import { invoke } from "@tauri-apps/api/core"

import type { AppTheme } from "@/lib/app-settings"
import type { SupportedLanguage } from "@/lib/i18n/resources"

export type AppPreferences = {
  theme: AppTheme
  language: SupportedLanguage
  defaultSearchRadiusKm: number
}

export function getAppPreferences() {
  return invoke<AppPreferences>("get_app_preferences")
}

export function setAppPreferences(preferences: AppPreferences) {
  return invoke<AppPreferences>("set_app_preferences", { preferences })
}

export function setAppTheme(theme: AppTheme) {
  return invoke<AppPreferences>("set_app_theme", { theme })
}

export function setAppLanguage(language: SupportedLanguage) {
  return invoke<AppPreferences>("set_app_language", { language })
}

export function setDefaultSearchRadiusKm(radiusKm: number) {
  return invoke<AppPreferences>("set_default_search_radius_km", { radiusKm })
}
