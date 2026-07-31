import { useCallback, useEffect, useState } from "react";

import { getSourceInventory } from "@/lib/api/sources";
import type {
  InstalledProfileWithDefinition,
  SourceInventory,
} from "@/lib/api/sources";

export function useSourceRegistryInventory() {
  const [data, setData] = useState<SourceInventory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const inventory = await getSourceInventory();
      const profiles = inventory.profiles.profiles.filter(
        (profile): profile is InstalledProfileWithDefinition =>
          profile.definition !== undefined,
      );
      const nextData: SourceInventory = {
        profiles,
        admittedProfiles: profiles.filter(
          (profile) => profile.admission === "admitted",
        ),
        sources: inventory.sources,
        diagnostics: [
          ...inventory.profiles.diagnostics,
          ...inventory.diagnostics,
        ],
      };
      setData(nextData);
      return nextData;
    } catch (unknownError) {
      const message = errorMessage(unknownError);
      setData(null);
      setError(message);
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { data, error, loading, refresh };
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
