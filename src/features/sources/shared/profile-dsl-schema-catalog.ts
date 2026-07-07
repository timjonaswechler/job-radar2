import commonSchema from "../../../../src-tauri/src/schema/profile-dsl/common.schema.json";
import diagnosticsSchema from "../../../../src-tauri/src/schema/profile-dsl/diagnostics.schema.json";
import extractSchema from "../../../../src-tauri/src/schema/profile-dsl/extract.schema.json";
import fetchSchema from "../../../../src-tauri/src/schema/profile-dsl/fetch.schema.json";
import overridesSchema from "../../../../src-tauri/src/schema/profile-dsl/overrides.schema.json";
import paginationSchema from "../../../../src-tauri/src/schema/profile-dsl/pagination.schema.json";
import parseSchema from "../../../../src-tauri/src/schema/profile-dsl/parse.schema.json";
import selectSchema from "../../../../src-tauri/src/schema/profile-dsl/select.schema.json";
import strategySchema from "../../../../src-tauri/src/schema/profile-dsl/strategy.schema.json";
import transformSchema from "../../../../src-tauri/src/schema/profile-dsl/transform.schema.json";
import sourceProfileSchema from "../../../../src-tauri/src/schema/source-profile.schema.json";
import sourceSchema from "../../../../src-tauri/src/schema/source.schema.json";

import {
  createSchemaCatalog,
  type SchemaCatalog,
} from "@/features/sources/shared/schema-introspection";
import type { JsonValue } from "@/lib/api/sources";

export const profileDslSchemaRefs = {
  source: "source.schema.json",
  sourceOwnedAccessPath: "source.schema.json#/$defs/sourceOwnedAccessPath",
  sourceOverrides: "profile-dsl/overrides.schema.json#/$defs/sourceOverrides",
  sourceProfile: "source-profile.schema.json",
  detection: "source-profile.schema.json#/$defs/detection",
  supportMetadata: "profile-dsl/common.schema.json#/$defs/supportMetadata",
  postingDiscoveryStep:
    "profile-dsl/strategy.schema.json#/$defs/postingDiscoveryStep",
  postingDetailStep: "profile-dsl/strategy.schema.json#/$defs/postingDetailStep",
} as const;

export const profileDslSchemaCatalog: SchemaCatalog = createSchemaCatalog([
  commonSchema,
  diagnosticsSchema,
  extractSchema,
  fetchSchema,
  overridesSchema,
  paginationSchema,
  parseSchema,
  selectSchema,
  strategySchema,
  transformSchema,
  sourceProfileSchema,
  sourceSchema,
] as JsonValue[]);
