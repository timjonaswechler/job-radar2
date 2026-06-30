import { invoke } from "@tauri-apps/api/core"

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue }

export type SourceStatus = "draft" | "active" | "disabled" | "invalid"

export type SourceKey = string

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
  supportsManualRelease: boolean
  authMode: AdapterAuthMode
  riskLevel: AdapterRiskLevel
}

export type SourceRegistryDocumentOrigin = "built_in" | "custom"

export type SourceRegistryDocumentKind = "source_profile" | "source"

export type SourceRegistryDiagnosticCode =
  | "invalid_json"
  | "invalid_shape"
  | "filename_key_mismatch"
  | "duplicate_key"
  | "missing_profile_ref"
  | "missing_path_ref"
  | "read_error"

export type SourceRegistryDiagnostic = {
  code: SourceRegistryDiagnosticCode
  documentKind: SourceRegistryDocumentKind
  origin: SourceRegistryDocumentOrigin
  path: string
  key: string | null
  message: string
}

export type SourceProfileKind =
  | "recruiting_system"
  | "job_portal"
  | "website_family"
  | "generic"

export type DetectionPhase = "http" | "browser"

export type DetectionBlock = {
  phases?: DetectionPhase[]
  required: JsonValue[]
}

export type SourceProfileIdentity = {
  keyCandidates?: string[]
  nameCandidates?: string[]
  optionalSourceConfig?: JsonValue
}

export type AvailabilityBlock = {
  requiredCaptures?: string[]
  checks?: JsonValue[]
  sourceConfig?: JsonValue
}

export type BrowserInteraction =
  | { type: "waitFor"; selector: string; timeoutMs?: number }
  | { type: "clickIfVisible"; selector: string; timeoutMs?: number }
  | {
      type: "clickUpToN"
      selector: string
      maxClicks: number
      waitAfterClickMs?: number
    }

export type ProfileAccessPathDefinition = {
  key: string
  name?: string
  adapterKey: string
  sourceConfigSchema?: JsonValue
  availability?: AvailabilityBlock
  query?: JsonValue
  inventory?: JsonValue
  postingDetail?: JsonValue
  interactions?: BrowserInteraction[]
  manualRelease?: JsonValue
}

export type SourceProfileDocument = {
  schemaVersion: 1
  key: string
  name: string
  kind: SourceProfileKind
  detect?: DetectionBlock
  identity?: SourceProfileIdentity
  sourceConfigSchema?: JsonValue
  accessPaths: ProfileAccessPathDefinition[]
}

export type ProfileSelectedAccessPath = {
  type: "profile"
  profileKey: string
  pathKey: string
}

export type SourceSpecificSelectedAccessPath = {
  type: "source_specific"
  adapterKey: string
  sourceConfigSchema?: JsonValue
  query?: JsonValue
  inventory?: JsonValue
  interactions?: BrowserInteraction[]
  manualRelease?: JsonValue
}

export type SelectedAccessPath =
  | ProfileSelectedAccessPath
  | SourceSpecificSelectedAccessPath

export type SourceDocument = {
  schemaVersion: 1
  key: SourceKey
  name: string
  status: SourceStatus
  sourceConfig: JsonValue
  selectedAccessPath: SelectedAccessPath
}

export type RegistrySourceProfile = {
  origin: SourceRegistryDocumentOrigin
  path: string
  document: SourceProfileDocument
}

export type RegistrySource = {
  origin: SourceRegistryDocumentOrigin
  path: string
  document: SourceDocument
}

export type SourceDetectionStatus =
  | "detected"
  | "ambiguous"
  | "unsupported"
  | "built_in_source"

export type SourceDetectionMatch = {
  adapterKey: string
  profileKey: string
  profileName: string
  pathKey: string
  pathName: string | null
  key: SourceKey
  name: string
  keyCandidates: string[]
  nameCandidates: string[]
  sourceConfig: JsonValue
  evidence: string[]
}

export type SourceDetectionResult = {
  status: SourceDetectionStatus
  adapterKey: string | null
  profileKey: string | null
  profileName: string | null
  pathKey: string | null
  pathName: string | null
  key: SourceKey | null
  name: string | null
  keyCandidates: string[]
  nameCandidates: string[]
  sourceConfig: JsonValue | null
  evidence: string[]
  warnings: string[]
  matches: SourceDetectionMatch[]
}

export function listAdapters() {
  return invoke<AdapterMetadata[]>("list_adapters")
}

export function listSourceRegistryProfiles() {
  return invoke<RegistrySourceProfile[]>("list_source_registry_profiles")
}

export function listSourceRegistrySources() {
  return invoke<RegistrySource[]>("list_source_registry_sources")
}

export function listSourceRegistryDiagnostics() {
  return invoke<SourceRegistryDiagnostic[]>(
    "list_source_registry_diagnostics",
  )
}

export function detectSourceFromUrl(url: string) {
  return invoke<SourceDetectionResult>("detect_source_from_url", { url })
}

export function createCustomSource(document: SourceDocument) {
  return invoke<RegistrySource>("create_custom_source", { document })
}
