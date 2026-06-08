import * as React from "react"

import { ThemeProviderContext } from "@/context/theme-provider-context"
import { setAppTheme, getAppPreferences } from "@/lib/api/app-preferences"
import { APP_SETTINGS, type AppTheme } from "@/lib/app-settings"
import {
  applyThemeToDocument,
  readStoredTheme,
  toggleTheme,
  writeStoredTheme,
} from "@/lib/theme"

type ThemeProviderProps = {
  children: React.ReactNode
  initialTheme?: AppTheme
}

export function ThemeProvider({ children, initialTheme }: ThemeProviderProps) {
  const [theme, setThemeState] = React.useState<AppTheme>(
    () => initialTheme ?? readStoredTheme()
  )

  const setTheme = React.useCallback((nextTheme: AppTheme) => {
    setThemeState(nextTheme)
    writeStoredTheme(nextTheme)
    applyThemeToDocument(nextTheme)
    void setAppTheme(nextTheme).catch((error) => {
      console.warn("Could not persist theme in SQLite", error)
    })
  }, [])

  const handleToggleTheme = React.useCallback(() => {
    setTheme(toggleTheme(theme))
  }, [setTheme, theme])

  React.useEffect(() => {
    applyThemeToDocument(theme)
  }, [theme])

  React.useEffect(() => {
    let cancelled = false

    void getAppPreferences()
      .then((preferences) => {
        if (cancelled) return
        setThemeState(preferences.theme)
        writeStoredTheme(preferences.theme)
        applyThemeToDocument(preferences.theme)
      })
      .catch((error) => {
        console.warn("Could not read theme from SQLite", error)
      })

    return () => {
      cancelled = true
    }
  }, [])

  React.useEffect(() => {
    const handleStorageChange = (event: StorageEvent) => {
      if (event.key !== APP_SETTINGS.theme.storageKey) return
      const nextTheme = readStoredTheme()
      setThemeState(nextTheme)
      applyThemeToDocument(nextTheme)
    }

    window.addEventListener("storage", handleStorageChange)
    return () => window.removeEventListener("storage", handleStorageChange)
  }, [])

  const value = React.useMemo(
    () => ({ theme, setTheme, toggleTheme: handleToggleTheme }),
    [handleToggleTheme, setTheme, theme]
  )

  return (
    <ThemeProviderContext.Provider value={value}>
      {children}
    </ThemeProviderContext.Provider>
  )
}
