import type { JsonValue } from "@/lib/api/sources";

export type JsonObject = { [key: string]: JsonValue };

export type SchemaResolutionOptions = {
  catalog?: SchemaCatalog;
  rootSchema?: JsonObject;
  baseUri?: string;
};

export type ResolvedSchema = {
  schema: JsonObject;
  rootSchema: JsonObject;
  baseUri?: string;
};

export type SchemaCatalog = {
  resolveRef: (
    ref: string,
    options?: Omit<SchemaResolutionOptions, "catalog">,
  ) => ResolvedSchema | undefined;
};

export type SchemaMetadata = {
  requiredKeys: Set<string>;
  properties: Map<string, JsonObject>;
};

export type SchemaFieldType = "string" | "number" | "boolean" | "json";

export type SchemaScalarOption = {
  value: string | number | boolean;
  label: string;
};

export type SchemaScalarRule = SchemaScalarOption & {
  kind: "const" | "enum";
};

export type SchemaVariant = {
  index: number;
  schema: JsonObject;
  label: string;
  score: number;
};

export function createSchemaCatalog(documents: JsonValue[]): SchemaCatalog {
  const documentsByKey = new Map<string, JsonObject>();

  for (const document of documents) {
    if (!isJsonObject(document)) continue;

    const id = typeof document.$id === "string" ? document.$id : undefined;
    if (id) {
      for (const key of documentKeys(id)) {
        documentsByKey.set(key, document);
      }
    }
  }

  return {
    resolveRef(ref, options) {
      return resolveRefFromCatalog(ref, documentsByKey, options);
    },
  };
}

export function schemaMetadataForObject(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): SchemaMetadata {
  const context = rootContext(schema, options);
  const requiredKeys = new Set<string>();
  const properties = new Map<string, JsonObject>();

  for (const objectSchema of flattenObjectSchemas(schema, context)) {
    const required = objectSchema.required;
    if (Array.isArray(required)) {
      for (const key of required) {
        if (typeof key === "string") requiredKeys.add(key);
      }
    }

    const schemaProperties = objectSchema.properties;
    if (isJsonObject(schemaProperties)) {
      for (const [key, value] of Object.entries(schemaProperties)) {
        const propertySchema = resolveSchema(value, context);
        if (propertySchema) properties.set(key, propertySchema);
      }
    }
  }

  return { requiredKeys, properties };
}

export function schemaFieldTypeFromSchema(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): SchemaFieldType {
  const context = rootContext(schema, options);
  const resolvedSchema = resolveSchema(schema, context);
  const type = normalizedSchemaType(resolvedSchema);

  if (type === "number" || type === "integer") return "number";
  if (type === "boolean") return "boolean";
  if (type === "object" || type === "array") return "json";

  const scalarSample = schemaConstValue(resolvedSchema) ?? firstEnumValue(resolvedSchema);
  if (typeof scalarSample === "number") return "number";
  if (typeof scalarSample === "boolean") return "boolean";
  if (Array.isArray(scalarSample) || isJsonObject(scalarSample)) return "json";

  return "string";
}

export function schemaDefaultValue(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): JsonValue | undefined {
  const context = rootContext(schema, options);
  const resolvedSchema = resolveSchema(schema, context);
  return resolvedSchema && "default" in resolvedSchema
    ? resolvedSchema.default
    : undefined;
}

export function schemaScalarOptions(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): SchemaScalarOption[] {
  const values = new Map<string | number | boolean, SchemaScalarOption>();
  for (const rule of schemaScalarRules(schema, options)) {
    if (!values.has(rule.value)) {
      values.set(rule.value, { value: rule.value, label: rule.label });
    }
  }
  return [...values.values()];
}

export function schemaScalarRules(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): SchemaScalarRule[] {
  const context = rootContext(schema, options);
  const resolvedSchema = resolveSchema(schema, context);
  if (!resolvedSchema) return [];

  const rules: SchemaScalarRule[] = [];
  const constValue = schemaConstValue(resolvedSchema);
  if (constValue !== undefined) {
    rules.push({ kind: "const", value: constValue, label: String(constValue) });
  }

  const enumValues = resolvedSchema.enum;
  if (Array.isArray(enumValues)) {
    for (const value of enumValues) {
      if (isScalarSchemaValue(value)) {
        rules.push({ kind: "enum", value, label: String(value) });
      }
    }
  }

  return rules;
}

export function schemaConstraints(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): string[] {
  const context = rootContext(schema, options);
  const resolvedSchema = resolveSchema(schema, context);
  if (!resolvedSchema) return [];

  const constraints: string[] = [];
  if (typeof resolvedSchema.format === "string") constraints.push(resolvedSchema.format);
  if (typeof resolvedSchema.pattern === "string") constraints.push("pattern");
  if (typeof resolvedSchema.minimum === "number") constraints.push(`min ${resolvedSchema.minimum}`);
  if (typeof resolvedSchema.maximum === "number") constraints.push(`max ${resolvedSchema.maximum}`);
  if (resolvedSchema.additionalProperties === false) constraints.push("closed");
  return constraints;
}

export function schemaForProperty(
  key: string,
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): JsonObject | undefined {
  const context = rootContext(schema, options);
  for (const objectSchema of flattenObjectSchemas(schema, context)) {
    const properties = objectSchema.properties;
    if (!isJsonObject(properties)) continue;

    const propertySchema = resolveSchema(properties[key], context);
    if (propertySchema) return propertySchema;
  }
  return undefined;
}

export function schemaForArrayItem(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): JsonObject | undefined {
  const context = rootContext(schema, options);
  const resolvedSchema = resolveSchema(schema, context);
  const itemSchema = resolveSchemaWithContext(resolvedSchema?.items, context);
  return itemSchema
    ? materializeSchemaReferences(itemSchema.schema, itemSchema.context)
    : undefined;
}

export function activeSchemaVariant(
  schema: JsonValue | undefined,
  value: JsonValue,
  options: SchemaResolutionOptions = {},
): SchemaVariant | undefined {
  const resolved = resolveSchemaWithContext(schema, rootContext(schema, options));
  const resolvedSchema = resolved?.schema;
  const context = resolved?.context ?? rootContext(schema, options);
  const variants = schemaVariants(resolvedSchema, context);
  if (!variants.length) return undefined;

  return variants
    .map((variant, index) => ({
      index,
      schema: variant,
      label: schemaLabel(variant, `Variante ${index + 1}`),
      score: schemaVariantScore(variant, value, context),
    }))
    .filter((variant) => variant.score > 0)
    .sort((left, right) => right.score - left.score)[0];
}

export function schemaForValue(
  schema: JsonValue | undefined,
  value: JsonValue,
  options: SchemaResolutionOptions = {},
): JsonObject | undefined {
  const resolved = resolveSchemaWithContext(schema, rootContext(schema, options));
  const resolvedSchema = resolved?.schema;
  const variant = activeSchemaVariant(resolvedSchema, value, resolved?.context);
  return variant?.schema ?? resolvedSchema;
}

export function resolveSchema(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
): JsonObject | undefined {
  const resolved = resolveSchemaWithContext(schema, options);
  return resolved
    ? materializeSchemaReferences(resolved.schema, resolved.context)
    : undefined;
}

function materializeSchemaReferences(
  schema: JsonObject,
  options: SchemaResolutionOptions,
): JsonObject {
  const materialized: JsonObject = { ...schema };
  if (Array.isArray(schema.oneOf)) {
    materialized.oneOf = schema.oneOf.map(
      (variant) => resolveSchema(variant, options) ?? variant,
    );
  }
  if (Array.isArray(schema.anyOf)) {
    materialized.anyOf = schema.anyOf.map(
      (variant) => resolveSchema(variant, options) ?? variant,
    );
  }
  return materialized;
}

function resolveSchemaWithContext(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions = {},
  seenRefs: Set<string> = new Set(),
): { schema: JsonObject; context: SchemaResolutionOptions } | undefined {
  if (!isJsonObject(schema)) return undefined;

  const ref = schema.$ref;
  if (typeof ref !== "string") return { schema, context: options };
  if (seenRefs.has(ref)) return { schema, context: options };

  const resolved = options.catalog?.resolveRef(ref, {
    rootSchema: options.rootSchema,
    baseUri: options.baseUri,
  });

  if (resolved) {
    return resolveSchemaWithContext(
      resolved.schema,
      {
        ...options,
        rootSchema: resolved.rootSchema,
        baseUri: resolved.baseUri,
      },
      new Set([...seenRefs, ref]),
    );
  }

  const localResolved = resolveLocalRef(ref, options.rootSchema ?? schema);
  if (!localResolved) return { schema, context: options };

  return resolveSchemaWithContext(
    localResolved.schema,
    {
      ...options,
      rootSchema: localResolved.rootSchema,
      baseUri: options.baseUri,
    },
    new Set([...seenRefs, ref]),
  );
}

export function isJsonObject(value: JsonValue | undefined): value is JsonObject {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function rootContext(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions,
): SchemaResolutionOptions {
  return {
    ...options,
    rootSchema: options.rootSchema ?? (isJsonObject(schema) ? schema : undefined),
  };
}

function flattenObjectSchemas(
  schema: JsonValue | undefined,
  options: SchemaResolutionOptions,
): JsonObject[] {
  const resolvedSchema = resolveSchema(schema, options);
  if (!resolvedSchema) return [];

  const activeSchemas = [resolvedSchema];
  const allOf = resolvedSchema.allOf;
  if (Array.isArray(allOf)) {
    for (const childSchema of allOf) {
      activeSchemas.push(...flattenObjectSchemas(childSchema, options));
    }
  }

  return activeSchemas;
}

function schemaVariants(
  schema: JsonObject | undefined,
  options: SchemaResolutionOptions,
): JsonObject[] {
  if (!schema) return [];
  const variantList = Array.isArray(schema.oneOf)
    ? schema.oneOf
    : Array.isArray(schema.anyOf)
      ? schema.anyOf
      : [];

  return variantList.flatMap((variant) => {
    const resolvedVariant = resolveSchema(variant, options);
    return resolvedVariant ? [resolvedVariant] : [];
  });
}

function schemaVariantScore(
  schema: JsonObject,
  value: JsonValue,
  options: SchemaResolutionOptions,
) {
  if (!isJsonObject(value)) return 0;

  let score = 0;
  const properties = schema.properties;
  if (isJsonObject(properties)) {
    for (const [key, propertySchema] of Object.entries(properties)) {
      if (!(key in value)) continue;
      const resolvedPropertySchema = resolveSchema(propertySchema, options);
      const propertyValue = value[key];
      const constValue = schemaConstValue(resolvedPropertySchema);

      if (constValue !== undefined) {
        if (propertyValue !== constValue) return 0;
        score += 20;
        continue;
      }

      const enumValues = resolvedPropertySchema?.enum;
      if (Array.isArray(enumValues) && enumValues.includes(propertyValue)) {
        score += 5;
      }
    }
  }

  const required = schema.required;
  if (Array.isArray(required)) {
    score += required.filter((key) => typeof key === "string" && key in value).length;
  }

  return score;
}

function resolveRefFromCatalog(
  ref: string,
  documentsByKey: Map<string, JsonObject>,
  options: Omit<SchemaResolutionOptions, "catalog"> = {},
): ResolvedSchema | undefined {
  const [rawDocumentRef, rawPointer = ""] = ref.split("#");
  const documentRef = rawDocumentRef || options.baseUri || "";
  const rootSchema = documentRef
    ? findDocument(documentRef, documentsByKey)
    : options.rootSchema;

  if (!rootSchema) return undefined;

  const schema = rawPointer ? readJsonPointer(rootSchema, rawPointer) : rootSchema;
  if (!isJsonObject(schema)) return undefined;

  const id = typeof rootSchema.$id === "string" ? rootSchema.$id : options.baseUri;
  return { schema, rootSchema, baseUri: id };
}

function resolveLocalRef(
  ref: string,
  rootSchema: JsonObject,
): ResolvedSchema | undefined {
  const [documentRef, rawPointer = ""] = ref.split("#");
  if (documentRef) return undefined;

  const schema = rawPointer ? readJsonPointer(rootSchema, rawPointer) : rootSchema;
  if (!isJsonObject(schema)) return undefined;

  const id = typeof rootSchema.$id === "string" ? rootSchema.$id : undefined;
  return { schema, rootSchema, baseUri: id };
}

function readJsonPointer(document: JsonValue, pointer: string): JsonValue | undefined {
  if (!pointer) return document;
  if (!pointer.startsWith("/")) return undefined;

  return pointer
    .slice(1)
    .split("/")
    .map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~"))
    .reduce<JsonValue | undefined>((current, part) => {
      if (current === undefined) return undefined;
      if (Array.isArray(current)) {
        const index = Number(part);
        return Number.isInteger(index) ? current[index] : undefined;
      }
      if (isJsonObject(current)) return current[part];
      return undefined;
    }, document);
}

function findDocument(
  ref: string,
  documentsByKey: Map<string, JsonObject>,
): JsonObject | undefined {
  const direct = documentsByKey.get(ref);
  if (direct) return direct;

  const normalized = normalizeDocumentKey(ref);
  const normalizedMatch = documentsByKey.get(normalized);
  if (normalizedMatch) return normalizedMatch;

  for (const [key, document] of documentsByKey) {
    if (key.endsWith(`/${normalized}`) || key.endsWith(normalized)) return document;
  }

  return undefined;
}

function documentKeys(id: string) {
  const normalized = normalizeDocumentKey(id);
  const keys = new Set([id, normalized]);
  const parts = normalized.split("/");
  const basename = parts[parts.length - 1];
  if (basename) keys.add(basename);
  return keys;
}

function normalizeDocumentKey(id: string) {
  try {
    const url = new URL(id);
    return url.pathname.replace(/^\/schemas\//, "").replace(/^\//, "");
  } catch {
    return id.replace(/^\.\//, "").replace(/^\//, "");
  }
}

function normalizedSchemaType(schema: JsonObject | undefined) {
  const type = schema?.type;
  return Array.isArray(type) ? type.find((item) => item !== "null") : type;
}

function firstEnumValue(schema: JsonObject | undefined): JsonValue | undefined {
  const enumValues = schema?.enum;
  return Array.isArray(enumValues) ? enumValues[0] : undefined;
}

function schemaConstValue(
  schema: JsonObject | undefined,
): string | number | boolean | undefined {
  if (!schema || !("const" in schema)) return undefined;
  return isScalarSchemaValue(schema.const) ? schema.const : undefined;
}

function schemaLabel(schema: JsonObject, fallback: string) {
  if (typeof schema.title === "string") return schema.title;

  const properties = schema.properties;
  if (isJsonObject(properties)) {
    for (const [key, propertySchema] of Object.entries(properties)) {
      const constValue = schemaConstValue(
        isJsonObject(propertySchema) ? propertySchema : undefined,
      );
      if (constValue === undefined) continue;
      if (key === "mode" && constValue === "http") return "HTTP fetch";
      if (key === "mode" && constValue === "browser") return "Browser fetch";
      return String(constValue);
    }
  }

  return fallback;
}

function isScalarSchemaValue(
  value: JsonValue,
): value is string | number | boolean {
  return (
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  );
}
