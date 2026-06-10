import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

export type BrowserRuntimeState =
  | "unsupported"
  | "notInstalled"
  | "installing"
  | "installed"
  | "updateRequired"
  | "invalid"

export type BrowserRuntimeStatus = {
  status: BrowserRuntimeState
  platform: string
  requiredVersion?: string
  installedVersion?: string
  installDir: string
  executablePath?: string
  error?: string
}

export type BrowserRuntimeInstallProgress = {
  installId: string
  phase:
    | "downloading"
    | "verifying"
    | "extracting"
    | "finalizing"
    | "completed"
    | "failed"
  downloadedBytes?: number
  totalBytes?: number
  message?: string
}

export type BrowserRuntimeCheckResult = {
  ok: boolean
  status: BrowserRuntimeStatus
  message: string
}

export const BROWSER_RUNTIME_INSTALL_PROGRESS_EVENT =
  "browser-runtime:install-progress"

export function getBrowserRuntimeStatus() {
  return invoke<BrowserRuntimeStatus>("get_browser_runtime_status")
}

export function installBrowserRuntime() {
  return invoke<BrowserRuntimeStatus>("install_browser_runtime")
}

export function uninstallBrowserRuntime() {
  return invoke<BrowserRuntimeStatus>("uninstall_browser_runtime")
}

export function checkBrowserRuntime() {
  return invoke<BrowserRuntimeCheckResult>("check_browser_runtime")
}

export function listenToBrowserRuntimeInstallProgress(
  handler: (progress: BrowserRuntimeInstallProgress) => void,
) {
  return listen<BrowserRuntimeInstallProgress>(
    BROWSER_RUNTIME_INSTALL_PROGRESS_EVENT,
    (event) => handler(event.payload),
  )
}
