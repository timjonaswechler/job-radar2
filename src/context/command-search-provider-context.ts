import * as React from "react"

export const COMMAND_SEARCH_KEYBOARD_SHORTCUT = "f"
export const COMMAND_SEARCH_KEYBOARD_SHORTCUT_LABEL = "⌘F"

type CommandSearchProviderState = {
  open: boolean
  query: string
  setOpen: (open: boolean) => void
  setQuery: (query: string) => void
  openCommandSearch: () => void
  closeCommandSearch: () => void
  toggleCommandSearch: () => void
}

export const CommandSearchProviderContext = React.createContext<
  CommandSearchProviderState | undefined
>(undefined)

export function useCommandSearch() {
  const context = React.useContext(CommandSearchProviderContext)

  if (context === undefined) {
    throw new Error(
      "useCommandSearch must be used within a CommandSearchProvider"
    )
  }

  return context
}
