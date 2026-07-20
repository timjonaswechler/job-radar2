import commonSchema from "../../../../src-tauri/src/schema/profile-dsl/common.schema.json";
import diagnosticsSchema from "../../../../src-tauri/src/schema/profile-dsl/diagnostics.schema.json";
import extractSchema from "../../../../src-tauri/src/schema/profile-dsl/extract.schema.json";
import fetchSchema from "../../../../src-tauri/src/schema/profile-dsl/fetch.schema.json";
import fragmentsSchema from "../../../../src-tauri/src/schema/profile-dsl/fragments.schema.json";
import policySchema from "../../../../src-tauri/src/schema/profile-dsl/policy.schema.json";
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
  accessPathFragments: "source.schema.json#/properties/accessPaths",
  sourceProfile: "source-profile.schema.json",
  detection: "source-profile.schema.json#/$defs/detection",
  supportMetadata: "profile-dsl/common.schema.json#/$defs/supportMetadata",
  discoveryStep: "profile-dsl/policy.schema.json#/$defs/discoveryStrategySet",
  detailStep: "profile-dsl/policy.schema.json#/$defs/detailStrategySet",
} as const;

export const profileDslSchemaCatalog: SchemaCatalog = createSchemaCatalog([
  commonSchema,
  diagnosticsSchema,
  extractSchema,
  fetchSchema,
  fragmentsSchema,
  paginationSchema,
  policySchema,
  parseSchema,
  selectSchema,
  strategySchema,
  transformSchema,
  sourceProfileSchema,
  sourceSchema,
] as JsonValue[]);
