import type { ReactNode } from "react"

import { CommandSearchProvider } from "@/context/command-search-provider"
import { LocaleProvider } from "@/context/locale-provider"
import { ThemeProvider } from "@/context/theme-provider"
import "@/lib/i18n/i18n"

type AppProvidersProps = {
  children: ReactNode
}

export function AppProviders({ children }: AppProvidersProps) {
  return (
    <ThemeProvider>
      <LocaleProvider>
        <CommandSearchProvider>{children}</CommandSearchProvider>
      </LocaleProvider>
    </ThemeProvider>
  )
}
