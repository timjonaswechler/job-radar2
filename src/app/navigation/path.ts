export const APP_ROUTE_CHANGE_EVENT = "job-radar-route-change";

export function isAppPathActive(pathname: string, basePath: string) {
  if (basePath === "/") return pathname === "/";
  return pathname === basePath || pathname.startsWith(`${basePath}/`);
}

export function navigateTo(url: string) {
  const destination = new URL(url, window.location.href);

  if (
    destination.origin !== window.location.origin ||
    destination.protocol !== window.location.protocol ||
    destination.host !== window.location.host
  ) {
    window.open(destination.href, "_blank", "noopener,noreferrer");
    return;
  }

  const nextUrl = `${destination.pathname}${destination.search}${destination.hash}`;
  const currentUrl = `${window.location.pathname}${window.location.search}${window.location.hash}`;
  if (currentUrl === nextUrl) return;

  const pathnameChanged = destination.pathname !== window.location.pathname;

  window.history.pushState(null, "", nextUrl);
  window.dispatchEvent(new Event(APP_ROUTE_CHANGE_EVENT));

  if (pathnameChanged) {
    window.scrollTo({ top: 0, left: 0, behavior: "auto" });
  }
}
