import type { JsonValue } from "@/lib/api/sources";

export type JsonObject = { [key: string]: JsonValue };

export type SourceConfigEntry = {
  id: string;
  key: string;
  value: string;
};

export type SchemaMetadata = {
  requiredKeys: Set<string>;
  properties: Map<string, JsonObject>;
};

export const defaultSourceConfigSchema: JsonValue = { type: "object" };

export function effectiveSourceConfigSchema(
  profileSchema: JsonValue | undefined,
  pathSchema: JsonValue | undefined,
): JsonValue {
  if (profileSchema && pathSchema) return { allOf: [profileSchema, pathSchema] };
  return profileSchema ?? pathSchema ?? defaultSourceConfigSchema;
}

export function sourceConfigSchemaMetadata(schema: JsonValue): SchemaMetadata {
  const requiredKeys = new Set<string>();
  const properties = new Map<string, JsonObject>();

  for (const objectSchema of flattenObjectSchemas(schema)) {
    const required = objectSchema.required;
    if (Array.isArray(required)) {
      for (const key of required) {
        if (typeof key === "string") requiredKeys.add(key);
      }
    }

    const schemaProperties = objectSchema.properties;
    if (isJsonObject(schemaProperties)) {
      for (const [key, value] of Object.entries(schemaProperties)) {
        if (isJsonObject(value)) properties.set(key, value);
      }
    }
  }

  return { requiredKeys, properties };
}

export function sourceConfigFromEntries(
  entries: Array<Pick<SourceConfigEntry, "key" | "value">>,
  schemaMetadata: SchemaMetadata,
): { value: JsonObject; errors: string[] } {
  const value: JsonObject = {};
  const errors: string[] = [];
  const seenKeys = new Set<string>();

  for (const entry of entries) {
    const key = entry.key.trim();
    const rawValue = entry.value.trim();

    if (!key && !rawValue) continue;
    if (!key) {
      errors.push("Ein Konfigurationswert hat keinen Key.");
      continue;
    }
    if (!rawValue && !schemaMetadata.requiredKeys.has(key)) {
      continue;
    }
    if (seenKeys.has(key)) {
      errors.push(`Der Konfigurations-Key „${key}“ ist doppelt vorhanden.`);
      continue;
    }

    seenKeys.add(key);
    const propertySchema = schemaMetadata.properties.get(key);
    const converted = convertConfigValue(key, entry.value, propertySchema);
    if (!converted.ok) {
      errors.push(converted.error);
      continue;
    }
    value[key] = converted.value;
  }

  for (const requiredKey of schemaMetadata.requiredKeys) {
    const requiredValue = value[requiredKey];
    if (
      requiredValue === undefined ||
      requiredValue === null ||
      (typeof requiredValue === "string" && !requiredValue.trim())
    ) {
      errors.push(`Pflichtwert „${requiredKey}“ fehlt.`);
    }
  }

  return { value, errors };
}

export function entriesWithSchemaHints(
  entries: SourceConfigEntry[],
  schemaMetadata: SchemaMetadata,
  createEntryId = createDefaultEntryId,
): SourceConfigEntry[] {
  const nextEntries = [...entries];
  const existingKeys = new Set(
    nextEntries.map((entry) => entry.key).filter(Boolean),
  );
  const hintedKeys = [
    ...schemaMetadata.requiredKeys,
    ...[...schemaMetadata.properties.entries()]
      .filter(([, schema]) => schemaDefaultValue(schema) !== undefined)
      .map(([key]) => key),
  ];

  for (const key of hintedKeys) {
    if (existingKeys.has(key)) continue;
    nextEntries.push({
      id: createEntryId(),
      key,
      value: jsonValueToInputValue(schemaDefaultValue(schemaMetadata.properties.get(key))),
    });
    existingKeys.add(key);
  }

  return nextEntries;
}

export function configEntriesFromJsonObject(
  value: JsonValue,
  createEntryId = createDefaultEntryId,
): SourceConfigEntry[] {
  if (!isJsonObject(value)) return [];
  return Object.entries(value).map(([key, entryValue]) => ({
    id: createEntryId(),
    key,
    value: jsonValueToInputValue(entryValue),
  }));
}

export function schemaFieldType(schema: JsonObject | undefined) {
  const type = schema?.type;
  const normalizedType = Array.isArray(type)
    ? type.find((item) => item !== "null")
    : type;

  if (normalizedType === "number" || normalizedType === "integer") {
    return "number";
  }
  if (normalizedType === "boolean") return "boolean";
  if (normalizedType === "object" || normalizedType === "array") return "json";
  return "string";
}

export function schemaDefaultValue(
  schema: JsonObject | undefined,
): JsonValue | undefined {
  return schema && "default" in schema ? schema.default : undefined;
}

export function jsonValueToInputValue(value: JsonValue | undefined): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

export function configEntryDescription(
  key: string,
  propertySchema: JsonObject | undefined,
  required: boolean,
) {
  const title =
    typeof propertySchema?.title === "string" ? propertySchema.title : key;
  const description =
    typeof propertySchema?.description === "string"
      ? propertySchema.description
      : null;
  const type = schemaFieldType(propertySchema);
  const typeLabel =
    type === "json"
      ? "JSON-Wert"
      : type === "number"
        ? "Zahl"
        : type === "boolean"
          ? "Boolean"
          : "Text";

  return [title, required ? "Pflicht" : null, typeLabel, description]
    .filter(Boolean)
    .join(" · ");
}

export function isJsonObject(value: JsonValue | undefined): value is JsonObject {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function convertConfigValue(
  key: string,
  rawValue: string,
  propertySchema: JsonObject | undefined,
): { ok: true; value: JsonValue } | { ok: false; error: string } {
  const fieldType = schemaFieldType(propertySchema);
  const trimmedValue = rawValue.trim();

  if (fieldType === "number") {
    const parsed = Number(trimmedValue);
    if (!Number.isFinite(parsed)) {
      return { ok: false, error: `„${key}“ muss eine Zahl sein.` };
    }
    return { ok: true, value: parsed };
  }

  if (fieldType === "boolean") {
    if (["true", "1", "ja", "yes"].includes(trimmedValue.toLocaleLowerCase("de"))) {
      return { ok: true, value: true };
    }
    if (["false", "0", "nein", "no"].includes(trimmedValue.toLocaleLowerCase("de"))) {
      return { ok: true, value: false };
    }
    return { ok: false, error: `„${key}“ muss true/false oder ja/nein sein.` };
  }

  if (fieldType === "json") {
    try {
      return { ok: true, value: JSON.parse(trimmedValue) as JsonValue };
    } catch {
      return { ok: false, error: `„${key}“ braucht einen gültigen JSON-Wert.` };
    }
  }

  return { ok: true, value: rawValue };
}

function flattenObjectSchemas(schema: JsonValue): JsonObject[] {
  if (!isJsonObject(schema)) return [];
  const schemas: JsonObject[] = [schema];
  const allOf = schema.allOf;
  if (Array.isArray(allOf)) {
    for (const childSchema of allOf) {
      schemas.push(...flattenObjectSchemas(childSchema));
    }
  }
  return schemas;
}

function createDefaultEntryId() {
  return crypto.randomUUID();
}
