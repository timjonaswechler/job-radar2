import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import { App } from "./App.js"
import { CommandSearchProvider } from "@/context/command-search-provider.js"
import { LocaleProvider } from "@/context/locale-provider.js"
import { ThemeProvider } from "@/context/theme-provider.js"
import "@/lib/i18n/i18n"
import { applyThemeToDocument, readStoredTheme } from "@/lib/theme"
import "@/styles/globals.css"

const initialTheme = readStoredTheme()
applyThemeToDocument(initialTheme)

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider initialTheme={initialTheme}>
      <LocaleProvider>
        <CommandSearchProvider>
          <App />
        </CommandSearchProvider>
      </LocaleProvider>
    </ThemeProvider>
  </StrictMode>
)
