import * as React from "react"

import {
  COMMAND_SEARCH_KEYBOARD_SHORTCUT,
  CommandSearchProviderContext,
} from "@/context/command-search-provider-context"

type CommandSearchProviderProps = {
  children: React.ReactNode
}

export function CommandSearchProvider({ children }: CommandSearchProviderProps) {
  const [open, setOpenState] = React.useState(false)
  const [query, setQuery] = React.useState("")

  const setOpen = React.useCallback((nextOpen: boolean) => {
    setOpenState(nextOpen)
    if (!nextOpen) setQuery("")
  }, [])

  const openCommandSearch = React.useCallback(() => {
    setOpen(true)
  }, [setOpen])

  const closeCommandSearch = React.useCallback(() => {
    setOpen(false)
  }, [setOpen])

  const toggleCommandSearch = React.useCallback(() => {
    setOpen(!open)
  }, [open, setOpen])

  React.useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.key.toLowerCase() === COMMAND_SEARCH_KEYBOARD_SHORTCUT &&
        (event.metaKey || event.ctrlKey)
      ) {
        event.preventDefault()
        toggleCommandSearch()
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [toggleCommandSearch])

  const value = React.useMemo(
    () => ({
      open,
      query,
      setOpen,
      setQuery,
      openCommandSearch,
      closeCommandSearch,
      toggleCommandSearch,
    }),
    [
      closeCommandSearch,
      open,
      openCommandSearch,
      query,
      setOpen,
      toggleCommandSearch,
    ]
  )

  return (
    <CommandSearchProviderContext.Provider value={value}>
      {children}
    </CommandSearchProviderContext.Provider>
  )
}
