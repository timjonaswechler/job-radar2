import * as React from "react"

import { getAppPreferences } from "@/lib/api/app-preferences"
import { APP_SETTINGS } from "@/lib/app-settings"
import {
  applyBaseFontSizeToDocument,
  readStoredBaseFontSizePx,
  writeStoredBaseFontSizePx,
} from "@/lib/font-size"

type FontSizeProviderProps = {
  children: React.ReactNode
}

export function FontSizeProvider({ children }: FontSizeProviderProps) {
  React.useLayoutEffect(() => {
    applyBaseFontSizeToDocument(readStoredBaseFontSizePx())

    let cancelled = false

    void getAppPreferences()
      .then((preferences) => {
        if (cancelled) return
        writeStoredBaseFontSizePx(preferences.baseFontSizePx)
        applyBaseFontSizeToDocument(preferences.baseFontSizePx)
      })
      .catch((error) => {
        console.warn("Could not read base font size from SQLite", error)
      })

    return () => {
      cancelled = true
    }
  }, [])

  React.useEffect(() => {
    const handleStorageChange = (event: StorageEvent) => {
      if (event.key !== APP_SETTINGS.baseFontSizePx.storageKey) return
      applyBaseFontSizeToDocument(readStoredBaseFontSizePx())
    }

    window.addEventListener("storage", handleStorageChange)
    return () => window.removeEventListener("storage", handleStorageChange)
  }, [])

  return children
}
