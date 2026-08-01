import commonSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/common.schema.json";
import diagnosticsSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/diagnostics.schema.json";
import extractSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/extract.schema.json";
import fetchSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/fetch.schema.json";
import fragmentsSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/fragments.schema.json";
import policySchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/policy.schema.json";
import paginationSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/pagination.schema.json";
import parseSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/parse.schema.json";
import selectSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/select.schema.json";
import strategySchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/strategy.schema.json";
import transformSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-behavior/transform.schema.json";
import sourceProfileSchema from "../../../../src-tauri/crates/source-profile-dsl/schema/source-profile.schema.json";
import sourceSchema from "../../../../src-tauri/crates/sources/schema/source.schema.json";

import {
  createSchemaCatalog,
  type SchemaCatalog,
} from "@/features/sources/shared/schema-introspection";
import type { JsonValue } from "@/lib/api/sources";

export const sourceBehaviorSchemaRefs = {
  source: "source.schema.json",
  sourceOwnedAccessPath: "source.schema.json#/$defs/sourceOwnedAccessPath",
  accessPathFragments: "source.schema.json#/properties/accessPaths",
  sourceProfile: "source-profile.schema.json",
  detection: "source-profile.schema.json#/$defs/detection",
  supportMetadata: "source-behavior/common.schema.json#/$defs/supportMetadata",
  discoveryStep: "source-behavior/policy.schema.json#/$defs/discoveryStrategySet",
  detailStep: "source-behavior/policy.schema.json#/$defs/detailStrategySet",
} as const;

export const sourceSchemaCatalog: SchemaCatalog = createSchemaCatalog([
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
