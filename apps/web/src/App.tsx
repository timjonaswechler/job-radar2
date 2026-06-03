import { useEffect, useState } from "react"

import { AppToaster } from "@/components/app/app-toaster"
import { CommandSearchDialog } from "@/components/command-search-dialog"
import { LanguageSwitcher } from "@/components/language-switcher"
import { AppSidebar } from "@/components/sidebar/app-sidebar"
import { ThemeSwitcher } from "@/components/sidebar/theme-switcher"
import { getAppRoute } from "@/navigation/app-routes"
import { APP_ROUTE_CHANGE_EVENT } from "@/navigation/path"
import { Separator } from "@/components/ui/separator"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"

export function App() {
  const [pathname, setPathname] = useState(() => window.location.pathname)
  const route = getAppRoute(pathname)
  const Content = route.Component

  useEffect(() => {
    const syncPathname = () => setPathname(window.location.pathname)

    window.addEventListener("popstate", syncPathname)
    window.addEventListener(APP_ROUTE_CHANGE_EVENT, syncPathname)

    return () => {
      window.removeEventListener("popstate", syncPathname)
      window.removeEventListener(APP_ROUTE_CHANGE_EVENT, syncPathname)
    }
  }, [])

  return (
    <>
      <AppToaster />
      <SidebarProvider
        style={
          {
            "--sidebar-width": "calc(var(--spacing) * 72)",
            "--header-height": "calc(var(--spacing) * 12)",
          } as React.CSSProperties
        }
      >
        <AppSidebar variant="inset" collapsible="icon" />
        <SidebarInset className="relative z-20 min-w-0 peer-data-[variant=inset]:border [--dashboard-header-height:--spacing(12)]">
          <header className="sticky top-0 z-50 flex h-12 shrink-0 items-center gap-2 overflow-hidden rounded-t-[inherit] border-b bg-background/50 backdrop-blur-md transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12">
            <div className="flex w-full min-w-0 items-center justify-between px-4 lg:px-6">
              <div className="flex items-center gap-1 lg:gap-2">
                <SidebarTrigger className="-ml-1" />
                <Separator
                  orientation="vertical"
                  className="mx-2 data-[orientation=vertical]:h-4 data-[orientation=vertical]:self-center"
                />
                <CommandSearchDialog />
              </div>
              <div className="flex items-center gap-2">
                <p className="hidden text-sm font-medium text-muted-foreground md:block">
                  {route.title}
                </p>
                <LanguageSwitcher />
                <ThemeSwitcher />
              </div>
            </div>
          </header>

          <main className="h-full min-w-0 p-4 md:p-4">
            <Content />
          </main>
        </SidebarInset>
      </SidebarProvider>
    </>
  )
}

export default App
