export * from "./profiles"
export * from "./installed"
export * from "./detection"
export * from "./live-check"

import { invoke } from "@tauri-apps/api/core"
import type { CreateSourceDraft, ReviseSourceDefinition, SavedSource, InactiveSourceStatus } from "./profiles"

export function createSource(draft: CreateSourceDraft) {
  return invoke<SavedSource>("create_source", { draft })
}
export function updateSource(revision: ReviseSourceDefinition) {
  return invoke<SavedSource>("update_source", { revision })
}
export function setSourceInactive(sourceKey: string, status: InactiveSourceStatus) {
  return invoke<SavedSource>("set_source_inactive", { sourceKey, status })
}
