import * as React from "react"

import { ThemeProviderContext } from "@/components/theme-provider-context"
import type { AppTheme } from "@/lib/app-settings"
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
  }, [])

  const handleToggleTheme = React.useCallback(() => {
    setTheme(toggleTheme(theme))
  }, [setTheme, theme])

  React.useEffect(() => {
    applyThemeToDocument(theme)
  }, [theme])

  React.useEffect(() => {
    const handleStorageChange = (event: StorageEvent) => {
      if (event.key !== "job-radar-theme") return
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
