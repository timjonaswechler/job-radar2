import { useEffect, useState } from "react"

import { AppNotifications } from "@/components/app/notifications"
import { LanguageSwitcher } from "@/components/language-switcher"
import { AppSidebar } from "@/components/sidebar/app-sidebar"
import { SearchDialog } from "@/components/sidebar/search-dialog"
import { ThemeSwitcher } from "@/components/sidebar/theme-switcher"
import { APP_SETTINGS } from "@/lib/app-settings"
import { cn } from "@/lib/utils"
import { getAppRoute } from "@/navigation/app-routes"
import { APP_ROUTE_CHANGE_EVENT } from "@/navigation/path"
import { Separator } from "@workspace/ui/components/separator"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@workspace/ui/components//sidebar"

const { layout } = APP_SETTINGS

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
      <AppNotifications />
      <SidebarProvider
        style={
          {
            "--sidebar-width": layout.sidebar.width,
          } as React.CSSProperties
        }
      >
        <AppSidebar
          variant={layout.sidebar.variant}
          collapsible={layout.sidebar.collapsible}
        />
        <SidebarInset
          className={cn(
            "relative z-20",
            layout.contentLayout === "centered" &&
              "*:mx-auto *:w-full *:max-w-screen-2xl",
            layout.sidebar.variant === "inset" &&
              "peer-data-[variant=inset]:border",
            "[--dashboard-header-height:--spacing(12)]"
          )}
        >
          <header
            className={cn(
              "flex h-12 shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12",
              layout.navbarStyle === "sticky" &&
                "sticky top-0 z-50 overflow-hidden rounded-t-[inherit] bg-background/50 backdrop-blur-md"
            )}
          >
            <div className="flex w-full items-center justify-between px-4 lg:px-6">
              <div className="flex items-center gap-1 lg:gap-2">
                <SidebarTrigger className="-ml-1" />
                <Separator
                  orientation="vertical"
                  className="mx-2 data-[orientation=vertical]:h-4 data-[orientation=vertical]:self-center"
                />
                <SearchDialog />
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

          <main className="h-full p-4 md:p-4">
            <Content />
          </main>
        </SidebarInset>
      </SidebarProvider>
    </>
  )
}

export default App
