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
  | "stable"
  | "best_effort"
  | "experimental"
  | "unsupported"

export type SupportEvidenceKind =
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
  timeoutMs: number
  expectStatus?: number
  contains?: string
  regex?: string
  evidence?: string
}

export type DetectionBrowserWait =
  | { type: "selector"; selector?: string; timeoutMs: number }
  | { type: "network_idle"; selector?: string; timeoutMs: number }

export type DetectionBrowserInteraction =
  | {
      type: "click_if_visible"
      selector: string
      maxCount: number
      waitAfterMs?: number
    }
  | {
      type: "click_until_gone"
      selector: string
      maxCount: number
      waitAfterMs?: number
    }

export type DetectionBrowserProbe = {
  key: string
  url: string
  timeoutMs: number
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

export type StrategyPolicy = { type: "first_accepted" }

export type Acceptance = {
  requiredFields?: string[]
  minDescriptionLength?: number
  minResults?: number
  maxErrorRatio?: number
}

export type BrowserWait =
  | { type: "selector"; selector?: string; timeoutMs: number }
  | { type: "network_idle"; selector?: string; timeoutMs: number }

export type BrowserInteraction =
  | { type: "click_if_visible"; selector: string; maxCount: number; waitAfterMs?: number }
  | { type: "click_until_gone"; selector: string; maxCount: number; waitAfterMs?: number }

export type RequestBody =
  | { type: "json"; value: JsonObject }
  | { type: "text"; value: string }
  | { type: "form"; fields: Record<string, string> }

export type Fetch =
  | { mode: "http"; method?: "GET" | "POST"; url: string; headers?: Record<string, string>; body?: RequestBody; timeoutMs: number }
  | { mode: "browser"; url: string; timeoutMs: number; waits?: BrowserWait[]; interactions?: BrowserInteraction[] }

export type Parse = { type: "json" | "xml" | "html" | "text"; charset?: string }

export type Select =
  | { type: "document" }
  | { type: "json_path"; jsonPath: string }
  | { type: "xml_element"; element: string }
  | { type: "xml_text"; textPath: string }
  | { type: "css"; selector: string }
  | { type: "sitemap_urls"; urlPattern?: string }

export type Transform =
  | { type: "trim" | "normalize_whitespace" | "html_to_text" | "url_decode" | "slug_to_title" | "dedupe" | "to_string" }
  | { type: "split"; separator: string; trimParts?: boolean; dropEmpty?: boolean }
  | { type: "join"; separator: string }
  | { type: "regex_replace"; pattern: string; replacement: string }

export type Cardinality = "one" | "first" | "optional" | "all"

type FieldExpressionOptions = { cardinality?: Cardinality; transforms?: Transform[] }
export type FieldExpression = FieldExpressionOptions & (
  | { type: "const"; value: JsonValue }
  | { type: "template"; template: string }
  | { type: "source_config" | "posting_meta" | "capture" | "item_field"; key: string }
  | { type: "json_path"; jsonPath: string }
  | { type: "xml_text"; textPath: string }
  | { type: "xml_element"; element: string }
  | { type: "css_text"; selector: string }
  | { type: "css_attribute"; selector: string; attribute: string }
  | { type: "combine"; parts: Array<{ value: FieldExpression; optional?: boolean }>; join?: string }
)

export type Filter =
  | { type: "non_empty"; field: FieldExpression }
  | { type: "regex"; field: FieldExpression; pattern: string }

export type Captures = Record<string, { from: FieldExpression; pattern: string }>
export type FieldMatch = { left: FieldExpression; right: FieldExpression }

export type PaginationLimits = { maxRequests?: number; maxItems?: number; maxDepth?: number }
export type PaginationParameterLocation = "query" | "json_body"
export type Pagination =
  | { type: "page"; pageParam: string; parameterLocation?: PaginationParameterLocation; firstPage?: number; pageSizeParam?: string; pageSize?: number; totalPath?: string; limits?: PaginationLimits }
  | { type: "offset_limit"; offsetParam: string; limitParam: string; parameterLocation?: PaginationParameterLocation; startOffset?: number; limit: number; totalPath?: string; limits?: PaginationLimits }
  | { type: "cursor"; cursorParam: string; parameterLocation?: PaginationParameterLocation; nextCursorPath: string; limits?: PaginationLimits }
  | { type: "sitemap"; childSitemapSelector?: Select; postingUrlSelector?: Select; limits?: PaginationLimits }

export type DiscoveryFields = {
  title: FieldExpression
  company: FieldExpression
  url: FieldExpression
  locations?: FieldExpression | FieldExpression[]
  postingMeta?: Record<string, FieldExpression>
  descriptionText?: FieldExpression
}

export type DetailFields = { descriptionText: FieldExpression }

export type DiscoveryStrategy = {
  key: string
  description?: string
  fetch: Fetch
  pagination?: Pagination
  parse: Parse
  select: Select
  where?: Filter[]
  captures?: Captures
  extract: { fields: DiscoveryFields }
  acceptWhen?: Acceptance
  diagnostics?: Diagnostics
}

export type DetailStrategy = {
  key: string
  description?: string
  fetch: Fetch
  parse: Parse
  select: Select
  where?: Filter[]
  captures?: Captures
  match?: FieldMatch
  extract: { fields: DetailFields }
  acceptWhen?: Acceptance
  diagnostics?: Diagnostics
}

export type DiscoveryStep = { policy: StrategyPolicy; strategies: DiscoveryStrategy[]; acceptWhen?: Acceptance }
export type DetailStep = { policy: StrategyPolicy; strategies: DetailStrategy[]; acceptWhen?: Acceptance }

export type RequestBodyFragment = { type?: "json" | "text" | "form"; value?: JsonValue; fields?: Record<string, string> }
export type FetchFragment = {
  mode?: "http" | "browser"
  method?: "GET" | "POST"
  url?: string
  headers?: Record<string, string>
  body?: RequestBodyFragment
  timeoutMs?: number
  waits?: BrowserWait[]
  interactions?: BrowserInteraction[]
}
export type ParseFragment = { type?: "json" | "xml" | "html" | "text"; charset?: string }
export type SelectFragment = {
  type?: "document" | "json_path" | "xml_element" | "xml_text" | "css" | "sitemap_urls"
  jsonPath?: string
  element?: string
  textPath?: string
  selector?: string
  urlPattern?: string
}
export type FieldExpressionFragment = {
  type?: "const" | "template" | "source_config" | "posting_meta" | "capture" | "item_field" | "json_path" | "xml_text" | "xml_element" | "css_text" | "css_attribute" | "combine"
  value?: JsonValue
  template?: string
  key?: string
  jsonPath?: string
  textPath?: string
  element?: string
  selector?: string
  attribute?: string
  parts?: Array<{ value?: FieldExpressionFragment; optional?: boolean }>
  join?: string
  cardinality?: Cardinality
  transforms?: Transform[]
}
export type PaginationFragment = {
  type?: "page" | "offset_limit" | "cursor" | "sitemap"
  pageParam?: string
  parameterLocation?: PaginationParameterLocation
  firstPage?: number
  pageSizeParam?: string
  pageSize?: number
  totalPath?: string
  offsetParam?: string
  limitParam?: string
  startOffset?: number
  limit?: number
  cursorParam?: string
  nextCursorPath?: string
  childSitemapSelector?: SelectFragment
  postingUrlSelector?: SelectFragment
  limits?: PaginationLimits
}
export type FilterFragment =
  | { type: "non_empty"; field: FieldExpression }
  | { type: "regex"; field: FieldExpression; pattern: string }
export type CapturesFragment = Record<string, { from?: FieldExpressionFragment; pattern?: string }>
export type DiscoveryStrategyFragment = {
  key: string
  fetch?: FetchFragment
  pagination?: PaginationFragment
  parse?: ParseFragment
  select?: SelectFragment
  where?: FilterFragment[]
  captures?: CapturesFragment
  extract?: { fields?: { title?: FieldExpressionFragment; company?: FieldExpressionFragment; url?: FieldExpressionFragment; locations?: FieldExpressionFragment | FieldExpressionFragment[]; postingMeta?: Record<string, FieldExpressionFragment>; descriptionText?: FieldExpressionFragment } }
  acceptWhen?: Acceptance
}
export type DetailStrategyFragment = {
  key: string
  fetch?: FetchFragment
  parse?: ParseFragment
  select?: SelectFragment
  where?: FilterFragment[]
  captures?: CapturesFragment
  match?: { left?: FieldExpressionFragment; right?: FieldExpressionFragment }
  extract?: { fields?: { descriptionText?: FieldExpressionFragment } }
  acceptWhen?: Acceptance
}
export type DiscoveryStepFragment = { policy?: StrategyPolicy; strategies?: DiscoveryStrategyFragment[]; acceptWhen?: Acceptance }
export type DetailStepFragment = { policy?: StrategyPolicy; strategies?: DetailStrategyFragment[]; acceptWhen?: Acceptance }

export type AccessPathFragment = {
  key: string
  name?: string
  sourceConfigSchema?: JsonObject
  discovery?: DiscoveryStepFragment
  detail?: DetailStepFragment
}

export type ProfileAccessPathDefinition = {
  key: string
  name: string
  description?: string
  sourceConfigSchema?: JsonObject
  knownIssues?: SupportNote[]
  discovery: DiscoveryStep
  detail?: DetailStep
  diagnostics?: Diagnostics
}

export type SourceProfileDocument = {
  schemaVersion: 3
  key: string
  name: string
  kind: SourceProfileKind
  description?: string
  support: SupportMetadata
  detection?: ProfileDetectionDocument
  sourceConfigSchema?: JsonObject
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
  sourceConfigSchema?: JsonObject
  discovery: DiscoveryStep
  detail?: DetailStep
  diagnostics?: Diagnostics
}

export type SelectedAccessPath =
  | ProfileSelectedAccessPath
  | SourceOwnedSelectedAccessPath

export type SourceDocument = {
  schemaVersion: 3
  key: SourceKey
  name: string
  status: SourceStatus
  sourceConfig: JsonObject
  selectedAccessPath: SelectedAccessPath
  accessPaths?: AccessPathFragment[]
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
  effectiveProfile?: SourceProfileDocument
}

export type CheckReportKind = "source_live_check"

export type CheckReportSubjectType = "source"

export type CheckReportResult = "passed" | "failed"

export type CheckReportSubject = {
  type: CheckReportSubjectType
  key: string
}

export type CheckFingerprint = {
  kind: string
  sha256: string
  reference?: string
}

export type CheckReport = {
  schemaVersion: 1
  kind: CheckReportKind
  subject: CheckReportSubject
  checkedAt: string
  logicVersion: string
  result: CheckReportResult
  fingerprints: CheckFingerprint[]
  diagnostics: Diagnostics
  details: JsonObject
}

export type CheckReportFreshnessState = "fresh" | "stale"

export type CheckReportStaleReason =
  | "logic_version_changed"
  | "missing_report_fingerprint"
  | "changed_fingerprint_sha256"
  | "unexpected_report_fingerprint"

export type CheckReportStaleDetail = {
  kind: string
  reference?: string
  reason: CheckReportStaleReason
  expectedSha256?: string
  actualSha256?: string
  expectedValue?: string
  actualValue?: string
}

export type CheckReportFreshness = {
  state: CheckReportFreshnessState
  staleFingerprints: CheckReportStaleDetail[]
}

export type SourceLiveCheckReportStatus = {
  state: "fresh" | "stale" | "unknown"
  report?: CheckReport | null
  freshness?: CheckReportFreshness | null
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

export function checkSource(sourceKey: string) {
  return invoke<CheckReport>("check_source", { sourceKey })
}

export function checkAndActivateSource(sourceKey: string) {
  return invoke<CheckReport>("check_and_activate_source", { sourceKey })
}

export function checkAndReactivateSource(sourceKey: string) {
  return invoke<CheckReport>("check_and_reactivate_source", { sourceKey })
}

export function getSourceLiveCheckReportStatus(sourceKey: string) {
  return invoke<SourceLiveCheckReportStatus>(
    "get_source_live_check_report_status",
    { sourceKey },
  )
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
