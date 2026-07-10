import { isJsonObject } from "@/features/sources/shared/schema-introspection";
import type {
  JsonValue,
  ProfileAccessPathDefinition,
} from "@/lib/api/sources";

export function sourceOverridesFromText(rawText: string): {
  value: JsonValue | null;
  errors: string[];
} {
  const trimmed = rawText.trim();
  if (!trimmed) return { value: null, errors: [] };

  try {
    const value = JSON.parse(trimmed) as JsonValue;
    if (!isJsonObject(value)) {
      return {
        value: null,
        errors: ["Source Overrides müssen ein JSON-Objekt sein."],
      };
    }
    return { value, errors: [] };
  } catch {
    return {
      value: null,
      errors: ["Source Overrides brauchen gültiges JSON."],
    };
  }
}

export function sourceOverridesStarterForAccessPath(
  accessPath: ProfileAccessPathDefinition | null,
): string {
  const target = firstStrategyOverrideTarget(accessPath);
  return JSON.stringify(
    {
      strategyOverrides: [
        {
          step: target?.step ?? "postingDiscovery",
          strategyKey: target?.strategyKey ?? "",
        },
      ],
    },
    null,
    2,
  );
}

function firstStrategyOverrideTarget(
  accessPath: ProfileAccessPathDefinition | null,
): { step: "postingDiscovery" | "postingDetail"; strategyKey: string } | null {
  const postingDiscoveryKey = firstStrategyKey(accessPath?.postingDiscovery);
  if (postingDiscoveryKey) {
    return { step: "postingDiscovery", strategyKey: postingDiscoveryKey };
  }

  const postingDetailKey = firstStrategyKey(accessPath?.postingDetail);
  if (postingDetailKey) {
    return { step: "postingDetail", strategyKey: postingDetailKey };
  }

  return null;
}

function firstStrategyKey(step: JsonValue | undefined): string | null {
  if (!isJsonObject(step)) return null;
  const strategies = step.strategies;
  if (!Array.isArray(strategies)) return null;

  for (const strategy of strategies) {
    if (!isJsonObject(strategy)) continue;
    const key = strategy.key;
    if (typeof key === "string" && key.trim()) return key;
  }
  return null;
}
