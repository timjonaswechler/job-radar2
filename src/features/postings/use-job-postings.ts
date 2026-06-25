import { useCallback, useEffect, useState } from "react";

import {
  listJobPostings,
  type JobPosting,
} from "@/lib/api/job-postings";

export function useJobPostings() {
  const [postings, setPostings] = useState<JobPosting[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const nextPostings = await listJobPostings();
      setPostings(nextPostings);
    } catch (unknownError) {
      setPostings([]);
      setError(errorMessage(unknownError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { postings, loading, error, refresh };
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
