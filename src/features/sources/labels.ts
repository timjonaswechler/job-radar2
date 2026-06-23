import type {
  AdapterMetadata,
  SourceProfileKind,
  SourceRegistryDiagnosticCode,
  SourceRegistryDocumentKind,
  SourceRegistryDocumentOrigin,
} from "@/lib/api/sources";

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
  generic: "Generisch",
};

export const diagnosticCodeLabels: Record<
  SourceRegistryDiagnosticCode,
  string
> = {
  invalid_json: "Ungültiges JSON",
  invalid_shape: "Ungültige Dokumentform",
  filename_key_mismatch: "Dateiname passt nicht zum Key",
  duplicate_key: "Doppelter Key",
  missing_profile_ref: "Fehlendes Profil",
  missing_path_ref: "Fehlender Zugriffspfad",
  read_error: "Lesefehler",
};

export const adapterExecutionModeLabels: Record<
  AdapterMetadata["executionMode"],
  string
> = {
  source_inventory: "Quellenbestand",
  query_parameterized: "Suchparameterisiert",
};
