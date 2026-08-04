import type { StructuredDiagnostic } from "@/lib/api/sources"

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

export function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}

export function isPositiveSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0
}

export function isArrayOf<T>(
  value: unknown,
  predicate: (entry: unknown) => entry is T,
): value is T[] {
  return Array.isArray(value) && value.every(predicate)
}

export function isString(value: unknown): value is string {
  return typeof value === "string"
}

export function isStructuredDiagnostic(value: unknown): value is StructuredDiagnostic {
  return (
    isRecord(value) &&
    ["schema", "registry", "compiler", "runtime", "detection", "source_validation"].includes(String(value.category)) &&
    typeof value.code === "string" &&
    typeof value.message === "string" &&
    ["info", "warning", "error"].includes(String(value.severity)) &&
    typeof value.path === "string" &&
    (value.strategyKey === undefined || typeof value.strategyKey === "string") &&
    (value.details === undefined || isJsonValue(value.details, 0))
  )
}

function isJsonValue(value: unknown, depth: number): boolean {
  if (depth > 32) return false
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean" ||
    (typeof value === "number" && Number.isFinite(value))
  ) {
    return true
  }
  if (Array.isArray(value)) {
    return value.every((entry) => isJsonValue(entry, depth + 1))
  }
  return (
    isRecord(value) &&
    Object.values(value).every((entry) => isJsonValue(entry, depth + 1))
  )
}
