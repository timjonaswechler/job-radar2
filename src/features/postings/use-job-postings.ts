import { useCallback, useEffect, useState } from "react";

import { listJobPostings, type JobPosting } from "@/lib/api/job-postings";

export type JobPostingsLoadError = {
  title: string;
  description: string;
};

export function useJobPostings() {
  const [postings, setPostings] = useState<JobPosting[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<JobPostingsLoadError | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const nextPostings = await listJobPostings();
      setPostings(nextPostings);
    } catch (unknownError) {
      setPostings([]);
      console.error("Failed to load job postings", unknownError);
      setError({
        title: "Stellenanzeigen konnten nicht geladen werden",
        description:
          "Die gespeicherten Anzeigen sind gerade nicht erreichbar. Prüfe, ob die lokale App-Datenbank verfügbar ist, und versuche es erneut.",
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { postings, loading, error, refresh };
}
