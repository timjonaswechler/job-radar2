export type WindowControlPlatform = "macos" | "windows" | "linux"

export function isWindowControlPlatform(
  value: string | null | undefined
): value is WindowControlPlatform {
  return value === "macos" || value === "windows" || value === "linux"
}

export function resolveWindowControlPlatform({
  navigatorPlatform,
  userAgent,
  override,
}: {
  navigatorPlatform: string
  userAgent: string
  override?: string | null
}): WindowControlPlatform {
  if (isWindowControlPlatform(override)) return override

  const normalizedPlatform = navigatorPlatform.toLowerCase()
  const normalizedUserAgent = userAgent.toLowerCase()

  if (
    normalizedPlatform.includes("mac") ||
    normalizedUserAgent.includes("mac os x")
  ) {
    return "macos"
  }
  if (
    normalizedPlatform.includes("win") ||
    normalizedUserAgent.includes("windows")
  ) {
    return "windows"
  }

  return "linux"
}
