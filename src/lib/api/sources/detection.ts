import { invoke } from "@tauri-apps/api/core"
import type { Diagnostics, DetectionEvidenceKind, JsonObject, SupportLevel } from "./profiles"

export type SourceProposalEvidence = {
  kind: DetectionEvidenceKind
  message: string
  path?: string
  descriptorPath: string
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
  provenance: DetectionProposalProvenance
}

export type DetectionOrigin = { strategyKey: string; schemaPath: string }
export type DetectionProposalProvenance = {
  captures: Record<string, DetectionOrigin[]>
  sourceConfig: Record<string, DetectionOrigin[]>
  recommendation: DetectionOrigin[]
  evidence: DetectionOrigin[][]
}

export type UnsupportedSourceProfile = {
  profileKey: string
  profileName: string
  supportLevel: SupportLevel
  captures: Record<string, string>
  evidence: SourceProposalEvidence[]
  provenance: DetectionProposalProvenance
}

export type SourceProposalDetectionStatus =
  | "matched"
  | "ambiguous"
  | "unsupported"
  | "failed"
  | "budget_exhausted"
  | "cancelled"

export type SourceProposalDetectionResult = {
  status: SourceProposalDetectionStatus
  proposals: SourceProposal[]
  unsupportedProfiles: UnsupportedSourceProfile[]
  diagnostics: Diagnostics
}

export type PhaseUsage = {
  strategyAttempts: number
  requests: number
  producedItems: number
  durationMs: number
  pages: number
  browserActions: number
  fanOut: number
  responseBytes: number
  browserRenderedBytes: number
}

export type PhaseExecutionReport = {
  usage: PhaseUsage
  completion:
    | { type: "accepted" }
    | { type: "policy_unsatisfied" }
    | { type: "execution_failed" }
    | { type: "cancelled"; reason: "user_cancelled" }
    | {
        type: "budget_exhausted"
        exhaustion: {
          dimension: string
          requested: number
          remaining: number
          limitSources: string[]
        }
      }
}

export type DetectionAttempt =
  | { type: "matched"; value: SourceProposal }
  | { type: "unsupported"; value: UnsupportedSourceProfile }
  | { type: "failed" | "conflict" | "budget_exhausted" | "cancelled"; value: Diagnostics }

export type DetectionProfileCompletion =
  | { type: "matched" | "unsupported" }
  | { type: "rejected"; strategyKey: string; kind: string }
  | { type: "execution_failed"; strategyKey?: string; kind: string | { type: string; kind?: string } }

export type DetectionProfileOutcome = {
  profileKey: string
  completion: DetectionProfileCompletion
  diagnostics: Diagnostics
}

export type DetectionOperationResult = SourceProposalDetectionResult & {
  diagnostics: Diagnostics
  report: PhaseExecutionReport
}


export function detectSourceProposalFromUrl(url: string) {
  return invoke<DetectionOperationResult>("detect_source_proposal_from_url", { url })
}
