import {
  APP_SETTINGS,
  readStoredWindowDragRegionEnabled,
  writeStoredWindowDragRegionEnabled,
} from "@/lib/app-settings"
import {
  isWindowControlPlatform,
  resolveWindowControlPlatform,
  type WindowControlPlatform,
} from "@/lib/window-control-platform"

export const WINDOW_DRAG_REGION_PREFERENCE_CHANGED_EVENT =
  "job-radar:window-drag-region-preference-changed"
export const WINDOW_CONTROL_PLATFORM_OVERRIDE_STORAGE_KEY =
  "job-radar-window-control-platform-override"
export const WINDOW_CONTROL_PLATFORM_OVERRIDE_QUERY_PARAM =
  "windowControlPlatform"

export function notifyWindowDragRegionPreferenceChanged(enabled: boolean) {
  window.dispatchEvent(
    new CustomEvent(WINDOW_DRAG_REGION_PREFERENCE_CHANGED_EVENT, {
      detail: { enabled },
    })
  )
}

export function readWindowControlPlatformOverride() {
  const queryValue = new URLSearchParams(window.location.search).get(
    WINDOW_CONTROL_PLATFORM_OVERRIDE_QUERY_PARAM
  )

  if (queryValue === "reset") {
    window.localStorage.removeItem(WINDOW_CONTROL_PLATFORM_OVERRIDE_STORAGE_KEY)
    return null
  }

  if (isWindowControlPlatform(queryValue)) {
    window.localStorage.setItem(
      WINDOW_CONTROL_PLATFORM_OVERRIDE_STORAGE_KEY,
      queryValue
    )
    return queryValue
  }

  const storedValue = window.localStorage.getItem(
    WINDOW_CONTROL_PLATFORM_OVERRIDE_STORAGE_KEY
  )
  return isWindowControlPlatform(storedValue) ? storedValue : null
}

export function detectWindowControlPlatform() {
  return resolveWindowControlPlatform({
    navigatorPlatform: window.navigator.platform,
    userAgent: window.navigator.userAgent,
    override: readWindowControlPlatformOverride(),
  })
}

export function applyWindowControlPlatform(platform: WindowControlPlatform) {
  document.documentElement.dataset.windowControlPlatform = platform
}

export function applyStoredWindowDragRegionEnabled(enabled: boolean) {
  writeStoredWindowDragRegionEnabled(enabled)
  notifyWindowDragRegionPreferenceChanged(enabled)
}

export function isWindowDragRegionStorageKey(key: string | null) {
  return key === APP_SETTINGS.windowDragRegionEnabled.storageKey
}

export function isWindowControlPlatformOverrideStorageKey(key: string | null) {
  return key === WINDOW_CONTROL_PLATFORM_OVERRIDE_STORAGE_KEY
}

export { readStoredWindowDragRegionEnabled }
