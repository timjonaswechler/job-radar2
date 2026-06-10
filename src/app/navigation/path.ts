export const APP_ROUTE_CHANGE_EVENT = "job-radar-route-change"

export function navigateTo(url: string) {
  if (url.startsWith("http://") || url.startsWith("https://")) {
    window.open(url, "_blank", "noopener,noreferrer")
    return
  }

  if (window.location.pathname === url) return

  window.history.pushState(null, "", url)
  window.dispatchEvent(new Event(APP_ROUTE_CHANGE_EVENT))
  window.scrollTo({ top: 0 })
}
