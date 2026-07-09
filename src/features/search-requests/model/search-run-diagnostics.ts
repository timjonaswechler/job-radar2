import type { StructuredDiagnostic } from "@/lib/api/sources";

export type SearchRunDiagnosticViewModel = {
  title: string;
  message: string;
  severity: StructuredDiagnostic["severity"];
  code: string;
  category: StructuredDiagnostic["category"];
  path: string;
  details: StructuredDiagnostic["details"];
};

const diagnosticCopy: Record<string, { title: string; message: string }> = {
  location_filter_not_applied_missing_radius_km: {
    title: "Standortfilter nicht angewendet",
    message:
      "Die Search Request enthält Locations, aber keinen gespeicherten Radius. Der Search Run bleibt reproduzierbar und wendet deshalb keinen Standortfilter an.",
  },
  location_filter_candidate_locations_unresolved: {
    title: "Einige Kandidaten-Orte konnten nicht aufgelöst werden",
    message:
      "Nicht auflösbare Location-Werte tragen nicht zum aktiven Standortfilter-Match bei. Details zeigen Anzahl und Beispiele.",
  },
  location_filter_ambiguous_locations: {
    title: "Mehrdeutige Ortsauflösung im Standortfilter",
    message:
      "Mindestens ein Ort wurde mehrfach aufgelöst. Der Search Run berücksichtigt aktuell alle aufgelösten Geo-Punkte.",
  },
};

export function createSearchRunDiagnosticViewModels(
  diagnostics: StructuredDiagnostic[],
): SearchRunDiagnosticViewModel[] {
  return diagnostics.map((diagnostic) => {
    const copy = diagnosticCopy[diagnostic.code];

    return {
      title: copy?.title ?? "Search-Run-Diagnostic",
      message: copy?.message ?? diagnostic.message,
      severity: diagnostic.severity,
      code: diagnostic.code,
      category: diagnostic.category,
      path: diagnostic.path,
      details: diagnostic.details,
    };
  });
}
