import { useCallback, useEffect, useState } from "react";

import { getSourceProfileRegistrySnapshot } from "@/lib/api/sources";
import type { SourceProfileRegistrySnapshot } from "@/lib/api/sources";

export function useSourceRegistryInventory() {
  const [data, setData] = useState<SourceProfileRegistrySnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const nextData = await getSourceProfileRegistrySnapshot();
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
