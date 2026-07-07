import { invoke } from "@tauri-apps/api/core"

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue }

export type JsonObject = { [key: string]: JsonValue }

export type SourceStatus = "draft" | "active" | "disabled"

export type SourceKey = string

export type SourceRegistryDocumentOrigin = "built_in" | "custom"

export type SourceRegistryDocumentKind = "source_profile" | "source"

export type SupportLevel =
  | "verified"
  | "best_effort"
  | "experimental"
  | "unsupported"

export type SupportEvidenceKind =
  | "fixture"
  | "smoke"
  | "manual_review"
  | "schema_check"

export type SupportNote = {
  message: string
  scope?: string
}

export type SupportEvidence = {
  kind: SupportEvidenceKind
  reference: string
  summary?: string
}

export type SupportMetadata = {
  level: SupportLevel
  summary?: string
  knownIssues?: SupportNote[]
  evidence?: SupportEvidence[]
}

export type StructuredDiagnostic = {
  category:
    | "schema"
    | "registry"
    | "compiler"
    | "runtime"
    | "detection"
    | "source_validation"
  code: string
  message: string
  severity: "info" | "warning" | "error"
  path: string
  strategyKey?: string
  details?: JsonValue
}

export type Diagnostics = StructuredDiagnostic[]

export type SourceProfileKind =
  | "recruiting_system"
  | "job_portal"
  | "website_family"
  | "career_site"
  | "generic"

export type DetectionEvidenceKind = "url" | "http" | "html" | "browser"

export type DetectionEvidence = {
  kind: DetectionEvidenceKind
  message: string
  path?: string
}

export type DetectionHttpCheck = {
  key: string
  url: string
  timeoutMs?: number
  expectStatus?: number
  contains?: string
  regex?: string
  evidence?: string
}

export type DetectionBrowserWait =
  | { type: "selector"; selector: string; timeoutMs?: number }
  | { type: "network_idle"; selector?: string; timeoutMs?: number }

export type DetectionBrowserInteraction =
  | {
      type: "click_if_visible"
      selector: string
      maxCount?: number
      waitAfterMs?: number
    }
  | {
      type: "click_until_gone"
      selector: string
      maxCount?: number
      waitAfterMs?: number
    }

export type DetectionBrowserProbe = {
  key: string
  url: string
  timeoutMs?: number
  waits?: DetectionBrowserWait[]
  interactions?: DetectionBrowserInteraction[]
  htmlContains?: string
  htmlRegex?: string
  evidence?: string
}

export type ProfileDetectionDocument = {
  inputUrlPatterns?: Array<{ pattern: string; captures?: string[] }>
  recommendedAccessPathKey?: string
  sourceConfig?: JsonObject
  keyCandidates?: string[]
  nameCandidates?: string[]
  httpChecks?: DetectionHttpCheck[]
  browserProbes?: DetectionBrowserProbe[]
  evidence?: DetectionEvidence[]
}

export type ProfileAccessPathDefinition = {
  key: string
  name: string
  description?: string
  sourceConfigSchema?: JsonValue
  knownIssues?: SupportNote[]
  postingDiscovery: JsonValue
  postingDetail?: JsonValue
  diagnostics?: Diagnostics
}

export type SourceProfileDocument = {
  schemaVersion: 2
  key: string
  name: string
  kind: SourceProfileKind
  description?: string
  support: SupportMetadata
  detect?: ProfileDetectionDocument
  sourceConfigSchema?: JsonValue
  accessPaths: ProfileAccessPathDefinition[]
  diagnostics?: Diagnostics
}

export type ProfileSelectedAccessPath = {
  type: "profile_access_path"
  profileKey: string
  pathKey: string
}

export type SourceOwnedSelectedAccessPath = {
  type: "source_owned_access_path"
  key: string
  name: string
  description?: string
  sourceConfigSchema?: JsonValue
  postingDiscovery: JsonValue
  postingDetail?: JsonValue
  diagnostics?: Diagnostics
}

export type SelectedAccessPath =
  | ProfileSelectedAccessPath
  | SourceOwnedSelectedAccessPath

export type SourceDocument = {
  schemaVersion: 2
  key: SourceKey
  name: string
  status: SourceStatus
  sourceConfig: JsonObject
  selectedAccessPath: SelectedAccessPath
  sourceOverrides?: JsonValue
  sourceSupport?: SupportMetadata
  diagnostics?: Diagnostics
}

export type ValidationStateKind = "unknown" | "valid" | "invalid"

export type SourceValidationState = {
  sourceKey: string
  state: ValidationStateKind
  canCompile: boolean
  canExecute: boolean
  diagnostics?: Diagnostics
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
  validationState: SourceValidationState
}

export type SourceProfileRegistrySnapshot = {
  profiles: RegistrySourceProfile[]
  sources: RegistrySource[]
  diagnostics: Diagnostics
}

export type SourceProposalEvidence = {
  kind: DetectionEvidenceKind
  message: string
  path?: string
  probeKey?: string
}

export type SourceProposal = {
  profileKey: string
  profileName: string
  recommendedAccessPathKey: string
  recommendedAccessPathName: string
  sourceConfig: JsonObject
  keyCandidates: string[]
  nameCandidates: string[]
  captures: Record<string, string>
  evidence: SourceProposalEvidence[]
  supportLevel: SupportLevel
}

export type UnsupportedSourceProfile = {
  profileKey: string
  profileName: string
  supportLevel: SupportLevel
  captures: Record<string, string>
  evidence: SourceProposalEvidence[]
}

export type SourceProposalDetectionStatus =
  | "matched"
  | "ambiguous"
  | "unsupported"
  | "failed"

export type SourceProposalDetectionResult = {
  status: SourceProposalDetectionStatus
  proposal?: SourceProposal
  proposals?: SourceProposal[]
  unsupportedProfiles?: UnsupportedSourceProfile[]
  diagnostics: Diagnostics
}

export function getSourceProfileRegistrySnapshot() {
  return invoke<SourceProfileRegistrySnapshot>(
    "get_source_profile_registry_snapshot",
  )
}

export function listSourceProfiles() {
  return invoke<RegistrySourceProfile[]>("list_source_profiles")
}

export function listSources() {
  return invoke<RegistrySource[]>("list_sources")
}

export function listSourceDiagnostics() {
  return invoke<Diagnostics>("list_source_diagnostics")
}

export function detectSourceProposalFromUrl(url: string) {
  return invoke<SourceProposalDetectionResult>("detect_source_proposal_from_url", { url })
}

export function createSource(document: SourceDocument) {
  return invoke<RegistrySource>("create_source", { document })
}

export function updateSource(document: SourceDocument) {
  return invoke<RegistrySource>("update_source", { document })
}
