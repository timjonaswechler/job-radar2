export type AppTheme = "light" | "dark"
export type ContentLayout = "centered" | "full-width"
export type NavbarStyle = "sticky" | "scroll"
export type SidebarVariant = "sidebar" | "inset" | "floating"
export type SidebarCollapsible = "icon" | "offcanvas"

export type AppSettings = {
  theme: {
    storageKey: string
    default: AppTheme
    values: readonly AppTheme[]
  }
  layout: {
    contentLayout: ContentLayout
    navbarStyle: NavbarStyle
    sidebar: {
      variant: SidebarVariant
      collapsible: SidebarCollapsible
      width: string
    }
  }
}

export const APP_SETTINGS: AppSettings = {
  theme: {
    storageKey: "job-radar-theme",
    default: "dark",
    values: ["light", "dark"],
  },
  layout: {
    contentLayout: "full-width",
    navbarStyle: "sticky",
    sidebar: {
      variant: "inset",
      collapsible: "icon",
      width: "calc(var(--spacing) * 68)",
    },
  },
}

export function isAppTheme(value: string | null): value is AppTheme {
  return APP_SETTINGS.theme.values.includes(value as AppTheme)
}
