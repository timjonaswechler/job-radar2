import { useCallback, useEffect, useState } from "react";

import { getDatabaseInfo, type DatabaseInfo } from "@/lib/api/database";

export function useDatabaseInfo() {
  const [data, setData] = useState<DatabaseInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      setData(await getDatabaseInfo());
    } catch (unknownError) {
      setData(null);
      setError(String(unknownError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { data, error, loading, refresh };
}
