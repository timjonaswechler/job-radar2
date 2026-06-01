import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import { App } from "./App.js"
import { ThemeProvider } from "@/components/theme-provider"
import "@/lib/i18n/i18n"
import { applyThemeToDocument, readStoredTheme } from "@/lib/theme"
import "@/styles/globals.css"

const initialTheme = readStoredTheme()
applyThemeToDocument(initialTheme)

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider initialTheme={initialTheme}>
      <App />
    </ThemeProvider>
  </StrictMode>
)
