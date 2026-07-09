import type {
  RegistrySource,
  RegistrySourceProfile,
  StructuredDiagnostic,
} from "@/lib/api/sources";

export {
  buildDiagnosticIndex,
  diagnosticDocumentKey,
  diagnosticDocumentKind,
  diagnosticDocumentOrigin,
  diagnosticDocumentPath,
} from "@/features/sources/view-model/diagnostics";
export type { DiagnosticIndex } from "@/features/sources/view-model/diagnostics";

export type SourceRegistryInventory = {
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  diagnostics: StructuredDiagnostic[];
};

export function diagnosticCountLabel(count: number) {
  return `${count} Diagnose${count === 1 ? "" : "n"}`;
}
