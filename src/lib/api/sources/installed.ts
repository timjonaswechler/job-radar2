import { invoke } from "@tauri-apps/api/core"
import type {
  Diagnostics,
  InstalledProfilesView,
  InstalledProfileWithDefinition,
  SourceDocument,
  SourceProfileDocument,
  SourceRegistryDocumentOrigin,
  SourceValidationState,
} from "./profiles"

export type RegistrySource = {
  origin: SourceRegistryDocumentOrigin
  path: string
  document: SourceDocument
  validationState: SourceValidationState
  effectiveProfile?: SourceProfileDocument
}

type SourceInventoryTransport = {
  profiles: InstalledProfilesView
  sources: RegistrySource[]
  diagnostics: Diagnostics
}

export type SourceInventory = {
  profiles: InstalledProfileWithDefinition[]
  admittedProfiles: InstalledProfileWithDefinition[]
  sources: RegistrySource[]
  diagnostics: Diagnostics
}

export function getSourceInventory() {
  return invoke<SourceInventoryTransport>("get_source_inventory")
}
