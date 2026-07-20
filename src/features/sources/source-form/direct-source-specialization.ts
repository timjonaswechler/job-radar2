import { isJsonObject } from "@/features/sources/shared/schema-introspection";
import type {
  AccessPathFragment,
  ProfileAccessPathDefinition,
} from "@/lib/api/sources";

export function directSourceSpecializationFromText(rawText: string): {
  value: AccessPathFragment[] | null;
  errors: string[];
} {
  const trimmed = rawText.trim();
  if (!trimmed) return { value: null, errors: [] };

  try {
    const value = JSON.parse(trimmed) as unknown;
    if (
      !Array.isArray(value) ||
      !value.every(
        (fragment) =>
          isJsonObject(fragment) &&
          typeof fragment.key === "string" &&
          fragment.key.trim().length > 0,
      )
    ) {
      return {
        value: null,
        errors: ["Direkte Source-Spezialisierung muss ein JSON-Array mit Access-Path-Keys sein."],
      };
    }
    return { value: value as AccessPathFragment[], errors: [] };
  } catch {
    return {
      value: null,
      errors: ["Direkte Source-Spezialisierung braucht gültiges JSON."],
    };
  }
}

export function directSourceSpecializationStarterForAccessPath(
  accessPath: ProfileAccessPathDefinition | null,
): string {
  const target = firstStrategyTarget(accessPath);
  const fragment: AccessPathFragment = {
    key: accessPath?.key ?? "",
    ...(target
      ? {
          [target.phase]: {
            strategies: [{ key: target.strategyKey }],
          },
        }
      : {}),
  };
  return JSON.stringify([fragment], null, 2);
}

function firstStrategyTarget(
  accessPath: ProfileAccessPathDefinition | null,
): { phase: "discovery" | "detail"; strategyKey: string } | null {
  const discoveryKey = accessPath?.discovery.strategies[0]?.key;
  if (discoveryKey) return { phase: "discovery", strategyKey: discoveryKey };

  const detailKey = accessPath?.detail?.strategies[0]?.key;
  if (detailKey) return { phase: "detail", strategyKey: detailKey };

  return null;
}
