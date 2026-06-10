import { invoke } from "@tauri-apps/api/core"

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue }

export type SourceStatus = "draft" | "active" | "disabled" | "invalid"

export type AdapterExecutionMode = "source_inventory" | "query_parameterized"

export type AdapterCategory = "job_board" | "generic" | "browser"

export type AdapterAuthMode = "none" | "manual_cookie"

export type AdapterRiskLevel = "stable" | "fragile" | "restricted"

export type AdapterMetadata = {
  key: string
  name: string
  description: string
  category: AdapterCategory
  executionMode: AdapterExecutionMode
  sourceConfigSchema: JsonValue
  requiresSystemProfile: boolean
  requiresBrowserProfile: boolean
  supportsManualRelease: boolean
  authMode: AdapterAuthMode
  riskLevel: AdapterRiskLevel
}

export type BrowserProfile = {
  id: number
  key: string
  name: string
  description: string | null
  nameI18nKey: string | null
  descriptionI18nKey: string | null
  definitionPath: string | null
  definitionHash: string | null
  definitionSchemaVersion: number
  definition: JsonValue
  sourceConfigSchema: JsonValue
  status: SourceStatus
  validationError: string | null
  createdAt: string
  updatedAt: string
}

export type CreateBrowserProfileInput = {
  key: string
  name: string
  description: string | null
  nameI18nKey: string | null
  descriptionI18nKey: string | null
  definitionPath: string | null
  definitionHash: string | null
  definitionSchemaVersion: number
  definition: JsonValue
  sourceConfigSchema: JsonValue
  status: SourceStatus
  validationError: string | null
}

export type UpdateBrowserProfileInput = Omit<CreateBrowserProfileInput, "key">

export type SystemProfile = {
  id: number
  key: string
  name: string
  description: string | null
  adapterKey: string
  definitionSchemaVersion: number
  definition: JsonValue
  sourceConfigSchema: JsonValue
  builtIn: boolean
  status: SourceStatus
  validationError: string | null
  createdAt: string
  updatedAt: string
}

export type CreateSystemProfileInput = {
  key: string
  name: string
  description: string | null
  adapterKey: string
  definitionSchemaVersion: number
  definition: JsonValue
  sourceConfigSchema: JsonValue
  status: SourceStatus
  validationError: string | null
}

export type UpdateSystemProfileInput = Omit<CreateSystemProfileInput, "key">

export type Source = {
  id: number
  key: string
  adapterKey: string
  systemProfileId: number | null
  browserProfileId: number | null
  name: string
  description: string | null
  sourceConfig: JsonValue
  status: SourceStatus
  validationError: string | null
  builtIn: boolean
  createdAt: string
  updatedAt: string
}

export type CreateSourceInput = {
  key: string
  adapterKey: string
  systemProfileId: number | null
  browserProfileId: number | null
  name: string
  description: string | null
  sourceConfig: JsonValue
  status: SourceStatus
  validationError: string | null
}

export type UpdateSourceInput = Omit<CreateSourceInput, "key">

export type SourceDetectionStatus =
  | "detected"
  | "ambiguous"
  | "unsupported"
  | "built_in_source"

export type SourceDetectionMatch = {
  adapterKey: string
  systemProfileId: number
  systemProfileKey: string
  systemProfileName: string
  key: string
  name: string
  sourceConfig: JsonValue
  evidence: string[]
}

export type SourceDetectionResult = {
  status: SourceDetectionStatus
  adapterKey: string | null
  systemProfileId: number | null
  systemProfileKey: string | null
  key: string | null
  name: string | null
  sourceConfig: JsonValue | null
  evidence: string[]
  warnings: string[]
  matches: SourceDetectionMatch[]
}

export function listAdapters() {
  return invoke<AdapterMetadata[]>("list_adapters")
}

export function detectSourceFromUrl(url: string) {
  return invoke<SourceDetectionResult>("detect_source_from_url", { url })
}

export function createBrowserProfile(input: CreateBrowserProfileInput) {
  return invoke<BrowserProfile>("create_browser_profile", { input })
}

export function listBrowserProfiles() {
  return invoke<BrowserProfile[]>("list_browser_profiles")
}

export function getBrowserProfile(id: number) {
  return invoke<BrowserProfile>("get_browser_profile", { id })
}

export function updateBrowserProfile(
  id: number,
  input: UpdateBrowserProfileInput,
) {
  return invoke<BrowserProfile>("update_browser_profile", { id, input })
}

export function deleteBrowserProfile(id: number) {
  return invoke<void>("delete_browser_profile", { id })
}

export function createSystemProfile(input: CreateSystemProfileInput) {
  return invoke<SystemProfile>("create_system_profile", { input })
}

export function listSystemProfiles() {
  return invoke<SystemProfile[]>("list_system_profiles")
}

export function getSystemProfile(id: number) {
  return invoke<SystemProfile>("get_system_profile", { id })
}

export function updateSystemProfile(id: number, input: UpdateSystemProfileInput) {
  return invoke<SystemProfile>("update_system_profile", { id, input })
}

export function deleteSystemProfile(id: number) {
  return invoke<void>("delete_system_profile", { id })
}

export function createSource(input: CreateSourceInput) {
  return invoke<Source>("create_source", { input })
}

export function listSources() {
  return invoke<Source[]>("list_sources")
}

export function getSource(id: number) {
  return invoke<Source>("get_source", { id })
}

export function updateSource(id: number, input: UpdateSourceInput) {
  return invoke<Source>("update_source", { id, input })
}

export function deleteSource(id: number) {
  return invoke<void>("delete_source", { id })
}
