import { useEffect, useState } from "react"

import { AppToaster } from "@/components/app/app-toaster"
import { AppLayout } from "@/components/layout/app-layout"
import { getAppRoute } from "@/navigation/app-routes"
import { APP_ROUTE_CHANGE_EVENT } from "@/navigation/path"

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
      <AppLayout title={route.title}>
        <Content />
      </AppLayout>
    </>
  )
}

export default App
