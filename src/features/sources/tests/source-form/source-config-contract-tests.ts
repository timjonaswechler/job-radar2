import assert from "node:assert/strict";

import {
  profileDslSchemaCatalog,
  profileDslSchemaRefs,
} from "@/features/sources/shared/profile-dsl-schema-catalog";
import {
  effectiveSourceConfigSchema,
  entriesWithSchemaHints,
  sourceConfigFromEntries,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import {
  sourceOverridesFromText,
  sourceOverridesStarterForAccessPath,
} from "@/features/sources/source-form/source-overrides";
import type { JsonValue, ProfileAccessPathDefinition } from "@/lib/api/sources";

const profileSchema: JsonValue = {
  type: "object",
  required: ["boardSlug"],
  properties: { boardSlug: { type: "string" } },
};
const pathSchema: JsonValue = {
  type: "object",
  required: ["tenant"],
  properties: {
    tenant: { type: "string", title: "Tenant" },
    pageSize: { type: "integer", default: 50 },
    includeArchived: { type: "boolean", default: false },
    headers: { type: "object" },
    locale: { type: "string", default: "de-DE" },
    optionalNote: { type: "string" },
  },
};

assert.equal(effectiveSourceConfigSchema(profileSchema, undefined), profileSchema);
assert.equal(effectiveSourceConfigSchema(undefined, pathSchema), pathSchema);
assert.deepEqual(effectiveSourceConfigSchema(undefined, undefined), { type: "object" });
const combinedSchema = effectiveSourceConfigSchema(profileSchema, pathSchema);
assert.deepEqual(combinedSchema, { allOf: [profileSchema, pathSchema] });

const metadata = sourceConfigSchemaMetadata(combinedSchema);
assert.deepEqual([...metadata.requiredKeys], ["boardSlug", "tenant"]);
assert.deepEqual([...metadata.properties.keys()], [
  "boardSlug",
  "tenant",
  "pageSize",
  "includeArchived",
  "headers",
  "locale",
  "optionalNote",
]);
assert.equal(metadata.properties.get("tenant")?.title, "Tenant");

const hintedEntries = entriesWithSchemaHints(
  [entry("existing-board", "boardSlug", "acme")],
  metadata,
  stableEntryIds("hint"),
);
assert.deepEqual(hintedEntries, [
  entry("existing-board", "boardSlug", "acme"),
  entry("00000000-0000-4000-8000-hint00000001", "tenant", ""),
  entry("00000000-0000-4000-8000-hint00000002", "pageSize", "50"),
  entry("00000000-0000-4000-8000-hint00000003", "includeArchived", "false"),
  entry("00000000-0000-4000-8000-hint00000004", "locale", "de-DE"),
]);

assert.deepEqual(
  sourceConfigFromEntries(
    [
      entry("board", "boardSlug", "acme"),
      entry("tenant", "tenant", "main"),
      entry("page-size", "pageSize", "25"),
      entry("include-archived", "includeArchived", "ja"),
      entry("headers", "headers", '{"Accept":"application/json"}'),
      entry("optional-empty", "optionalNote", ""),
      entry("blank", "", ""),
    ],
    metadata,
  ),
  {
    value: {
      boardSlug: "acme",
      tenant: "main",
      pageSize: 25,
      includeArchived: true,
      headers: { Accept: "application/json" },
    },
    errors: [],
  },
);

const duplicateConfigKeys = sourceConfigFromEntries(
  [
    entry("board", "boardSlug", "acme"),
    entry("tenant", "tenant", "main"),
    entry("duplicate", "boardSlug", "other"),
  ],
  metadata,
);
assert.deepEqual(duplicateConfigKeys.value, { boardSlug: "acme", tenant: "main" });
assert.deepEqual(duplicateConfigKeys.errors, [
  "Der Konfigurations-Key „boardSlug“ ist doppelt vorhanden.",
]);
const invalidJsonConfig = sourceConfigFromEntries(
  [
    entry("board", "boardSlug", "acme"),
    entry("tenant", "tenant", "main"),
    entry("headers", "headers", "{not-json}"),
  ],
  metadata,
);
assert.deepEqual(invalidJsonConfig.value, { boardSlug: "acme", tenant: "main" });
assert.deepEqual(invalidJsonConfig.errors, [
  "„headers“ braucht einen gültigen JSON-Wert.",
]);
assert.deepEqual(
  sourceConfigFromEntries([entry("tenant", "tenant", "main")], metadata).errors,
  ["Pflichtwert „boardSlug“ fehlt."],
);

const preflightOnlyMetadata = sourceConfigSchemaMetadata({
  type: "object",
  required: ["host"],
  additionalProperties: false,
  properties: {
    host: { type: "string", pattern: "^example\\.com$" },
    mode: { type: "string", enum: ["live"] },
  },
});
assert.deepEqual(
  sourceConfigFromEntries(
    [
      entry("host", "host", "not-example.test"),
      entry("mode", "mode", "preview"),
      entry("extra", "extra", "kept-for-compiler-validation"),
    ],
    preflightOnlyMetadata,
  ),
  {
    value: {
      host: "not-example.test",
      mode: "preview",
      extra: "kept-for-compiler-validation",
    },
    errors: [],
  },
);

assert.deepEqual(
  sourceOverridesFromText(
    '{"strategyOverrides":[{"step":"postingDiscovery","strategyKey":"posting_api"}]}',
  ),
  {
    value: {
      strategyOverrides: [
        { step: "postingDiscovery", strategyKey: "posting_api" },
      ],
    },
    errors: [],
  },
);
assert.deepEqual(sourceOverridesFromText("   "), { value: null, errors: [] });
assert.deepEqual(sourceOverridesFromText("[]").errors, [
  "Source Overrides müssen ein JSON-Objekt sein.",
]);
assert.deepEqual(sourceOverridesFromText("{not-json}").errors, [
  "Source Overrides brauchen gültiges JSON.",
]);
const sourceOverridesSchema = profileDslSchemaCatalog.resolveRef(
  profileDslSchemaRefs.sourceOverrides,
);
assert.equal(sourceOverridesSchema?.schema.type, "object");
assert.ok(sourceOverridesSchema?.schema.properties);

const accessPath: ProfileAccessPathDefinition = {
  key: "posting_api",
  name: "Posting API",
  postingDiscovery: { strategies: [{ key: "posting_api" }] },
};
assert.equal(
  sourceOverridesStarterForAccessPath(accessPath),
  JSON.stringify(
    {
      strategyOverrides: [
        { step: "postingDiscovery", strategyKey: "posting_api" },
      ],
    },
    null,
    2,
  ),
);
assert.equal(
  sourceOverridesStarterForAccessPath(null),
  JSON.stringify(
    {
      strategyOverrides: [{ step: "postingDiscovery", strategyKey: "" }],
    },
    null,
    2,
  ),
);

function entry(id: string, key: string, value: string): SourceConfigEntry {
  return { id, key, value };
}

function stableEntryIds(prefix: string) {
  let nextId = 0;
  return () => `00000000-0000-4000-8000-${prefix}${String(++nextId).padStart(8, "0")}`;
}
