import type { ReactNode } from "react"

import { CommandSearchProvider } from "@/context/command-search-provider"
import { LocaleProvider } from "@/context/locale-provider"
import { ThemeProvider } from "@/context/theme-provider"
import { TooltipProvider } from "@/components/ui/tooltip"
import "@/lib/i18n/i18n"

type AppProvidersProps = {
  children: ReactNode
}

export function AppProviders({ children }: AppProvidersProps) {
  return (
    <ThemeProvider>
      <LocaleProvider>
        <TooltipProvider>
          <CommandSearchProvider>{children}</CommandSearchProvider>
        </TooltipProvider>
      </LocaleProvider>
    </ThemeProvider>
  )
}
