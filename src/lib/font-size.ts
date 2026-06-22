import { APP_SETTINGS, isBaseFontSizePx } from "@/lib/app-settings"

export function readStoredBaseFontSizePx() {
  const storedValue = window.localStorage.getItem(
    APP_SETTINGS.baseFontSizePx.storageKey
  )
  const baseFontSizePx = Number(storedValue)

  return isBaseFontSizePx(baseFontSizePx)
    ? baseFontSizePx
    : APP_SETTINGS.baseFontSizePx.default
}

export function writeStoredBaseFontSizePx(baseFontSizePx: number) {
  if (!isBaseFontSizePx(baseFontSizePx)) return

  window.localStorage.setItem(
    APP_SETTINGS.baseFontSizePx.storageKey,
    String(baseFontSizePx)
  )
}

export function applyBaseFontSizeToDocument(baseFontSizePx: number) {
  if (!isBaseFontSizePx(baseFontSizePx)) return

  document.documentElement.style.setProperty(
    "--job-radar-base-font-size",
    `${baseFontSizePx}px`
  )
}
