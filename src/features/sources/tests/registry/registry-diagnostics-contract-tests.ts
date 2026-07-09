import assert from "node:assert/strict";

import { DiagnosticCard } from "@/features/sources/registry/diagnostics/diagnostic-card";
import { InlineDiagnostics } from "@/features/sources/registry/diagnostics/inline-diagnostics";
import { buildDiagnosticIndex } from "@/features/sources/view-model/diagnostics";
import { SourcesDiagnosticsTab } from "@/features/sources/workspace/sources-diagnostics-tab";
import {
  parseSourcesWorkspaceTab,
  sourcesWorkspaceTabUrl,
} from "@/features/sources/workspace/sources-workspace-tabs";
import { useSourceRegistryInventory } from "@/features/sources/workspace/use-source-registry-inventory";
import type {
  RegistrySource,
  RegistrySourceProfile,
  StructuredDiagnostic,
} from "@/lib/api/sources";

assert.equal(typeof DiagnosticCard, "function");
assert.equal(typeof InlineDiagnostics, "function");
assert.equal(typeof SourcesDiagnosticsTab, "function");
assert.equal(typeof useSourceRegistryInventory, "function");
assert.equal(parseSourcesWorkspaceTab(""), "sources");
assert.equal(parseSourcesWorkspaceTab("?tab=sources"), "sources");
assert.equal(parseSourcesWorkspaceTab("?tab=profiles"), "profiles");
assert.equal(parseSourcesWorkspaceTab("?tab=diagnostics"), "diagnostics");
assert.equal(parseSourcesWorkspaceTab("?tab=runtime"), "runtime");
assert.equal(parseSourcesWorkspaceTab("?tab=unknown"), "sources");
assert.equal(
  sourcesWorkspaceTabUrl("runtime", {
    pathname: "/sources",
    search: "?filter=active&tab=unknown",
    hash: "#registry",
  }),
  "/sources?filter=active&tab=runtime#registry",
);

const source = registrySource("acme");
const profile = registryProfile("greenhouse");
const sourceDiagnostic = diagnostic("missing_access_path", {
  sourceKey: "acme",
});
const profileDiagnostic = diagnostic("profile_known_issue", {
  sourceProfileKey: "greenhouse",
});
const unassignedDiagnostic = diagnostic("unassigned_registry_warning", {
  key: "unknown_document",
});

const index = buildDiagnosticIndex(
  [source],
  [profile],
  [sourceDiagnostic, profileDiagnostic, unassignedDiagnostic],
);
assert.deepEqual(index.bySourceKey.get("acme"), [sourceDiagnostic]);
assert.deepEqual(index.byProfileKey.get("greenhouse"), [profileDiagnostic]);
assert.deepEqual(index.unassigned, [unassignedDiagnostic]);

function registrySource(key: string): RegistrySource {
  return {
    origin: "custom",
    path: `sources/${key}.json`,
    document: {
      schemaVersion: 2,
      key,
      name: key,
      status: "active",
      sourceConfig: {},
      selectedAccessPath: {
        type: "profile_access_path",
        profileKey: "greenhouse",
        pathKey: "boards_api",
      },
    },
    validationState: {
      sourceKey: key,
      state: "valid",
      canCompile: true,
      canExecute: true,
      diagnostics: [],
    },
  };
}

function registryProfile(key: string): RegistrySourceProfile {
  return {
    origin: "built_in",
    path: `profiles/${key}.json`,
    document: {
      schemaVersion: 2,
      key,
      name: key,
      kind: "recruiting_system",
      support: { level: "stable" },
      accessPaths: [],
    },
  };
}

function diagnostic(
  code: string,
  details: StructuredDiagnostic["details"],
): StructuredDiagnostic {
  return {
    category: "registry",
    code,
    message: code,
    severity: "warning",
    path: "/registry",
    details,
  };
}
