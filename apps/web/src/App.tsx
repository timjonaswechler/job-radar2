import { useTranslation } from "react-i18next"
import { siGithub } from "simple-icons"

import { LanguageSwitcher } from "@/components/language-switcher"
import { SimpleIcon } from "@/components/simple-icon"
import { AppSidebar } from "@/components/sidebar/app-sidebar"
import { SearchDialog } from "@/components/sidebar/search-dialog"
import { ThemeSwitcher } from "@/components/sidebar/theme-switcher"
import { APP_SETTINGS } from "@/lib/app-settings"
import { cn } from "@/lib/utils"
import { Button } from "@workspace/ui/components//button"
import { Separator } from "@workspace/ui/components/separator"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@workspace/ui/components//sidebar"

const { layout } = APP_SETTINGS

export function App() {
  const { t } = useTranslation()

  return (
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
              <LanguageSwitcher />
              <ThemeSwitcher />
              <Button
                size="icon"
                type="button"
                aria-label={t("app.openGithubRepository")}
                onClick={() =>
                  window.open(
                    "https://github.com/arhamkhnz/next-shadcn-admin-dashboard",
                    "_blank",
                    "noopener,noreferrer"
                  )
                }
              >
                <SimpleIcon
                  icon={siGithub}
                  className="fill-primary-foreground"
                />
              </Button>
            </div>
          </div>
        </header>

        <main className="h-full p-4 md:p-4">
          <section className="rounded-lg border bg-card p-6 text-card-foreground shadow-sm">
            <h1 className="text-2xl font-semibold">{t("app.title")}</h1>
            <p className="mt-2 text-sm text-muted-foreground">
              {t("app.description")}
            </p>
          </section>
        </main>
      </SidebarInset>
    </SidebarProvider>
  )
}

export default App
