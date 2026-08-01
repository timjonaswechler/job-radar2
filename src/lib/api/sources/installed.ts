import { invoke } from "@tauri-apps/api/core";
import type {
  Diagnostics,
  InstalledProfilesView,
  InstalledProfileWithDefinition,
  JsonValue,
  SourceDocument,
  SourceRegistryDocumentOrigin,
  SourceValidationState,
  SupportMetadata,
} from "./profiles";

export type ResolvedSourceBehavior = {
  accessPathName: string;
  profileSourceConfigSchema?: JsonValue;
  accessPathSourceConfigSchema?: JsonValue;
  support?: SupportMetadata;
  capabilities: string[];
};

/** Intentional installed Source projection. It never contains a filesystem
 * path, generation, compiler outcome/plan, or Effective Source Profile copy. */
export type InstalledSource = {
  origin: SourceRegistryDocumentOrigin;
  fileName: string;
  document: SourceDocument;
  validationState: SourceValidationState;
  resolved?: ResolvedSourceBehavior;
};

export type SourceInventoryTransport = {
  profiles: InstalledProfilesView;
  sources: InstalledSource[];
  diagnostics: Diagnostics;
};

export type SourceInventory = {
  profiles: InstalledProfileWithDefinition[];
  admittedProfiles: InstalledProfileWithDefinition[];
  sources: InstalledSource[];
  diagnostics: Diagnostics;
};

export async function getSourceInventory(): Promise<SourceInventoryTransport> {
  return decodeInventory(await invoke<unknown>("get_source_inventory"));
}

export function decodeInventory(value: unknown): SourceInventoryTransport {
  if (!isRecord(value) || !Array.isArray(value.sources)) {
    throw new Error("invalid installed Source inventory transport");
  }
  decodeProfiles(value.profiles);
  decodeDiagnostics(value.diagnostics, "inventory diagnostics");
  for (const source of value.sources) decodeInstalledSource(source);
  return value as SourceInventoryTransport;
}

export function decodeInstalledSource(source: unknown): InstalledSource {
  if (
    !isRecord(source) ||
    typeof source.fileName !== "string" ||
    (source.origin !== "built_in" && source.origin !== "custom")
  ) {
    throw new Error("invalid installed Source transport");
  }
  for (const forbidden of [
    "path",
    "effectiveProfile",
    "compileOutcome",
    "generation",
    "plan",
  ]) {
    if (forbidden in source)
      throw new Error(
        `installed Source transport exposes forbidden ${forbidden}`,
      );
  }
  const document = decodeSourceDocument(source.document);
  decodeValidationState(source.validationState, document.key);
  if (source.resolved !== undefined) decodeResolved(source.resolved);
  return source as InstalledSource;
}

function decodeSourceDocument(value: unknown): SourceDocument {
  if (
    !isRecord(value) ||
    value.schemaVersion !== 3 ||
    typeof value.key !== "string" ||
    typeof value.name !== "string" ||
    !["draft", "active", "disabled"].includes(String(value.status)) ||
    !isJsonObject(value.sourceConfig) ||
    !isRecord(value.selectedAccessPath)
  ) {
    throw new Error("invalid installed Source document");
  }
  const selected = value.selectedAccessPath;
  if (selected.type === "profile_access_path") {
    if (
      typeof selected.profileKey !== "string" ||
      typeof selected.pathKey !== "string"
    ) {
      throw new Error("invalid profile Access Path selection");
    }
  } else if (selected.type === "source_owned_access_path") {
    if (
      typeof selected.key !== "string" ||
      typeof selected.name !== "string"
    ) {
      throw new Error("invalid Source-owned Access Path selection");
    }
    if (selected.description !== undefined && typeof selected.description !== "string") {
      throw new Error("invalid Source-owned Access Path description");
    }
    if (selected.sourceConfigSchema !== undefined && !isJsonObject(selected.sourceConfigSchema)) {
      throw new Error("invalid Source-owned Source Config schema");
    }
    decodeStrategySet(selected.discovery, "Source-owned Discovery Strategy Set", false);
    if (selected.detail !== undefined) {
      decodeStrategySet(selected.detail, "Source-owned Detail Strategy Set", false);
    }
    if (selected.diagnostics !== undefined) {
      decodeDiagnostics(selected.diagnostics, "Source-owned Access Path diagnostics");
    }
  } else {
    throw new Error("invalid selected Access Path type");
  }
  if (value.accessPaths !== undefined) {
    if (!Array.isArray(value.accessPaths)) {
      throw new Error("invalid direct Source specialization");
    }
    for (const path of value.accessPaths) {
      if (!isRecord(path) || typeof path.key !== "string") {
        throw new Error("invalid direct Source specialization Access Path");
      }
      if (path.name !== undefined && typeof path.name !== "string") {
        throw new Error("invalid direct Source specialization Access Path name");
      }
      if (path.sourceConfigSchema !== undefined && !isJsonObject(path.sourceConfigSchema)) {
        throw new Error("invalid direct Source specialization schema");
      }
      if (path.discovery !== undefined) {
        decodeStrategySet(path.discovery, "direct Discovery Strategy Set", true);
      }
      if (path.detail !== undefined) {
        decodeStrategySet(path.detail, "direct Detail Strategy Set", true);
      }
    }
  }
  if (value.sourceSupport !== undefined) decodeSupport(value.sourceSupport);
  if (value.diagnostics !== undefined)
    decodeDiagnostics(value.diagnostics, "Source diagnostics");
  return value as SourceDocument;
}

function decodeValidationState(
  value: unknown,
  sourceKey: string,
): SourceValidationState {
  if (
    !isRecord(value) ||
    value.sourceKey !== sourceKey ||
    !["unknown", "valid", "invalid"].includes(String(value.state)) ||
    typeof value.canCompile !== "boolean" ||
    typeof value.canExecute !== "boolean"
  ) {
    throw new Error("invalid Source Validation State");
  }
  if (value.diagnostics !== undefined)
    decodeDiagnostics(value.diagnostics, "validation diagnostics");
  return value as SourceValidationState;
}

function decodeResolved(value: unknown): ResolvedSourceBehavior {
  if (
    !isRecord(value) ||
    typeof value.accessPathName !== "string" ||
    !Array.isArray(value.capabilities) ||
    value.capabilities.some((item) => typeof item !== "string")
  ) {
    throw new Error("invalid resolved Source behavior");
  }
  for (const schema of [
    value.profileSourceConfigSchema,
    value.accessPathSourceConfigSchema,
  ]) {
    if (schema !== undefined && !isJsonObject(schema))
      throw new Error("invalid resolved Source schema");
  }
  if (value.support !== undefined) decodeSupport(value.support);
  return value as ResolvedSourceBehavior;
}

function decodeProfiles(value: unknown): InstalledProfilesView {
  if (!isRecord(value) || !Array.isArray(value.profiles)) {
    throw new Error("invalid installed Profiles view");
  }
  decodeDiagnostics(value.diagnostics, "Profile diagnostics");
  for (const profile of value.profiles) {
    if (
      !isRecord(profile) ||
      (profile.origin !== "built_in" && profile.origin !== "custom") ||
      (profile.admission !== "admitted" && profile.admission !== "rejected") ||
      typeof profile.fileName !== "string"
    ) {
      throw new Error("invalid installed Profile");
    }
    if (profile.definition !== undefined) {
      const definition = profile.definition;
      if (
        !isRecord(definition) ||
        typeof definition.key !== "string" ||
        typeof definition.name !== "string" ||
        ![
          "recruiting_system",
          "job_portal",
          "website_family",
          "career_site",
          "generic",
        ].includes(String(definition.kind)) ||
        !Array.isArray(definition.accessPaths)
      ) {
        throw new Error("invalid installed Profile definition");
      }
      decodeSupport(definition.support);
      if (definition.description !== undefined && typeof definition.description !== "string") {
        throw new Error("invalid installed Profile description");
      }
      if (definition.sourceConfigSchema !== undefined && !isJsonObject(definition.sourceConfigSchema)) {
        throw new Error("invalid installed Profile Source Config schema");
      }
      if (definition.detection !== undefined) {
        if (!isRecord(definition.detection)) {
          throw new Error("invalid Profile Detection Strategy Set");
        }
        decodeStrategySet(definition.detection, "Profile Detection Strategy Set", false);
        decodeDetectionEvidence(definition.detection);
      }
      for (const path of definition.accessPaths) decodeProfileAccessPath(path);
    }
  }
  return value as InstalledProfilesView;
}

function decodeDetectionEvidence(value: Record<string, unknown>): void {
  if (value.evidence === undefined) return;
  if (!Array.isArray(value.evidence)) {
    throw new Error("invalid Profile Detection evidence");
  }
  for (const evidence of value.evidence) {
    if (
      !isRecord(evidence) ||
      !["url", "http", "html", "browser"].includes(String(evidence.kind)) ||
      typeof evidence.message !== "string" ||
      (evidence.path !== undefined && typeof evidence.path !== "string")
    ) {
      throw new Error("invalid Profile Detection evidence");
    }
  }
}

function decodeProfileAccessPath(value: unknown): void {
  if (
    !isRecord(value) ||
    typeof value.key !== "string" ||
    typeof value.name !== "string"
  ) {
    throw new Error("invalid installed Profile Access Path");
  }
  if (value.description !== undefined && typeof value.description !== "string") {
    throw new Error("invalid installed Profile Access Path description");
  }
  if (value.sourceConfigSchema !== undefined && !isJsonObject(value.sourceConfigSchema)) {
    throw new Error("invalid installed Profile Access Path schema");
  }
  if (value.knownIssues !== undefined) decodeSupportNotes(value.knownIssues);
  decodeStrategySet(value.discovery, "Profile Discovery Strategy Set", false);
  if (value.detail !== undefined) {
    decodeStrategySet(value.detail, "Profile Detail Strategy Set", false);
  }
  if (value.diagnostics !== undefined) {
    decodeDiagnostics(value.diagnostics, "Profile Access Path diagnostics");
  }
}

function decodeStrategySet(value: unknown, label: string, partial: boolean): void {
  if (!isRecord(value)) throw new Error(`invalid ${label}`);
  if (value.policy !== undefined) {
    if (!isRecord(value.policy) || ![
      "first_accepted",
      "all_required",
      "at_least",
      "collect_all",
    ].includes(String(value.policy.type))) {
      throw new Error(`invalid ${label} policy`);
    }
  } else if (!partial) {
    throw new Error(`invalid ${label} policy`);
  }
  if (value.strategies === undefined) {
    if (partial) return;
    throw new Error(`invalid ${label} strategies`);
  }
  if (!Array.isArray(value.strategies)) throw new Error(`invalid ${label} strategies`);
  for (const strategy of value.strategies) {
    if (!isRecord(strategy) || typeof strategy.key !== "string") {
      throw new Error(`invalid ${label} strategy`);
    }
  }
}

function decodeSupportNotes(value: unknown): void {
  if (!Array.isArray(value)) throw new Error("invalid Support Metadata knownIssues");
  for (const note of value) {
    if (
      !isRecord(note) ||
      typeof note.message !== "string" ||
      (note.scope !== undefined && typeof note.scope !== "string")
    ) {
      throw new Error("invalid Support Metadata knownIssues");
    }
  }
}

function decodeSupport(value: unknown): SupportMetadata {
  if (
    !isRecord(value) ||
    !["stable", "best_effort", "experimental", "unsupported"].includes(
      String(value.level),
    ) ||
    (value.summary !== undefined && typeof value.summary !== "string")
  ) {
    throw new Error("invalid Support Metadata");
  }
  if (value.knownIssues !== undefined) decodeSupportNotes(value.knownIssues);
  if (value.evidence !== undefined) {
    if (!Array.isArray(value.evidence)) throw new Error("invalid Support Metadata evidence");
    for (const evidence of value.evidence) {
      if (
        !isRecord(evidence) ||
        !["smoke", "manual_review", "schema_check"].includes(String(evidence.kind)) ||
        typeof evidence.reference !== "string" ||
        (evidence.summary !== undefined && typeof evidence.summary !== "string")
      ) {
        throw new Error("invalid Support Metadata evidence");
      }
    }
  }
  return value as SupportMetadata;
}

function decodeDiagnostics(value: unknown, label: string): Diagnostics {
  if (!Array.isArray(value)) throw new Error(`invalid ${label}`);
  for (const diagnostic of value) {
    if (
      !isRecord(diagnostic) ||
      ![
        "schema",
        "registry",
        "compiler",
        "runtime",
        "detection",
        "source_validation",
      ].includes(String(diagnostic.category)) ||
      typeof diagnostic.code !== "string" ||
      typeof diagnostic.message !== "string" ||
      !["info", "warning", "error"].includes(String(diagnostic.severity)) ||
      typeof diagnostic.path !== "string" ||
      (diagnostic.strategyKey !== undefined && typeof diagnostic.strategyKey !== "string") ||
      (diagnostic.details !== undefined && !isJsonValue(diagnostic.details))
    ) {
      throw new Error(`invalid ${label}`);
    }
  }
  return value as Diagnostics;
}

function isJsonObject(value: unknown): value is Record<string, JsonValue> {
  return isRecord(value) && Object.values(value).every(isJsonValue);
}

function isJsonValue(value: unknown): value is JsonValue {
  if (value === null || ["string", "number", "boolean"].includes(typeof value))
    return true;
  if (Array.isArray(value)) return value.every(isJsonValue);
  return isJsonObject(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
