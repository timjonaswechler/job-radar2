import type {
  CheckReportResult,
  DetectionEvidenceKind,
  EffectiveVerificationState,
  SourceProfileKind,
  SourceRegistryDocumentKind,
  SourceRegistryDocumentOrigin,
  SupportEvidenceKind,
  SupportLevel,
  ValidationStateKind,
} from "@/lib/api/sources";

export type ProfileVerificationDisplayState =
  | "verified"
  | "failed"
  | "stale"
  | "unknown"
  | "not_applicable";

export type SourceLiveCheckDisplayState =
  | "passed"
  | "failed"
  | "stale"
  | "unknown";

export const originLabels: Record<SourceRegistryDocumentOrigin, string> = {
  built_in: "Eingebaut",
  custom: "Custom",
};

export const documentKindLabels: Record<SourceRegistryDocumentKind, string> = {
  source_profile: "Quellenprofil",
  source: "Quelle",
};

export const documentDirectoryLabels: Record<SourceRegistryDocumentKind, string> = {
  source_profile: "source-profiles/*.json",
  source: "sources/*.json",
};

export const profileKindLabels: Record<SourceProfileKind, string> = {
  recruiting_system: "Recruiting-System",
  job_portal: "Job-Portal",
  website_family: "Website-Familie",
  career_site: "Karriere-Website",
  generic: "Generisch",
};

export const supportLevelLabels: Record<SupportLevel, string> = {
  verified: "Verifiziert",
  best_effort: "Best Effort",
  experimental: "Experimentell",
  unsupported: "Nicht unterstützt",
};

export const supportEvidenceKindLabels: Record<SupportEvidenceKind, string> = {
  fixture: "Fixture",
  smoke: "Smoke",
  manual_review: "Manual Review",
  schema_check: "Schema Check",
};

export const detectionEvidenceKindLabels: Record<DetectionEvidenceKind, string> = {
  url: "URL",
  http: "HTTP",
  html: "HTML",
  browser: "Browser",
};

export const validationStateLabels: Record<ValidationStateKind, string> = {
  unknown: "Unbekannt",
  valid: "Valide",
  invalid: "Ungültig",
};

export const checkReportResultLabels: Record<CheckReportResult, string> = {
  passed: "Bestanden",
  failed: "Fehlgeschlagen",
};

export const effectiveVerificationStateLabels: Record<EffectiveVerificationState, string> = {
  verified: "Effektiv verifiziert",
  failed: "Effektiv fehlgeschlagen",
  unknown: "Unbekannt",
  not_applicable: "Nicht anwendbar",
};

export const profileVerificationReportStateLabels: Record<"fresh" | "stale" | "unknown", string> = {
  fresh: "Frisch",
  stale: "Stale",
  unknown: "Unbekannt",
};

export const profileVerificationDisplayStateLabels: Record<ProfileVerificationDisplayState, string> = {
  verified: "Verifiziert",
  failed: "Fehlgeschlagen",
  stale: "Stale",
  unknown: "Unbekannt",
  not_applicable: "Nicht anwendbar",
};

export const sourceLiveCheckReportStateLabels: Record<"fresh" | "stale" | "unknown", string> = {
  fresh: "Frisch",
  stale: "Stale",
  unknown: "Unbekannt",
};

export const sourceLiveCheckDisplayStateLabels: Record<SourceLiveCheckDisplayState, string> = {
  passed: "Live-Prüfung bestanden",
  failed: "Live-Prüfung fehlgeschlagen",
  stale: "Stale",
  unknown: "Unbekannt",
};

export function diagnosticCodeLabel(code: string) {
  return code
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toLocaleUpperCase("de") + part.slice(1))
    .join(" ");
}
