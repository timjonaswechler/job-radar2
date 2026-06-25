import { useEffect, useMemo, useState } from "react";

import type { JobPosting } from "@/lib/api/job-postings";
import {
  getPostingInboxAnchorFromPath,
  getPostingQueueIdFromPath,
  getQueueDefinition,
  isPostingInQueue,
} from "@/features/postings/postings-view-model";

import { PostingsList } from "./postings-list";
import { PostingsPreview } from "./postings-preview";

type PostingsProps = {
  error: string | null;
  loading: boolean;
  postings: JobPosting[];
  onRefresh: () => Promise<void>;
};

export function Postings({
  error,
  loading,
  postings,
  onRefresh,
}: PostingsProps) {
  const pathname = window.location.pathname;
  const activeQueueId = getPostingQueueIdFromPath(pathname);
  const activeInboxAnchorId = getPostingInboxAnchorFromPath(pathname);
  const [selectedPostingId, setSelectedPostingId] = useState<number | null>(
    null,
  );

  const activeQueue = getQueueDefinition(activeQueueId);
  const activeQueuePostings = useMemo(
    () =>
      postings.filter((posting) => isPostingInQueue(posting, activeQueueId)),
    [activeQueueId, postings],
  );

  useEffect(() => {
    if (loading) return;

    if (!activeQueuePostings.length) {
      setSelectedPostingId(null);
      return;
    }

    const selectedPostingIsVisible = activeQueuePostings.some(
      (posting) => posting.id === selectedPostingId,
    );

    if (!selectedPostingIsVisible) {
      setSelectedPostingId(activeQueuePostings[0].id);
    }
  }, [activeQueuePostings, loading, selectedPostingId]);

  const selectedPosting =
    activeQueuePostings.find((posting) => posting.id === selectedPostingId) ??
    null;

  return (
    <div className="grid flex-1 grid-cols-1 overflow-hidden shadow-sm transition-[grid-template-columns] lg:grid-cols-[minmax(0,24rem)_1fr]">
      <PostingsList
        activeInboxAnchorId={activeInboxAnchorId}
        activeQueue={activeQueue}
        error={error}
        loading={loading}
        postings={activeQueuePostings}
        selectedPostingId={selectedPostingId}
        onRetry={onRefresh}
        onSelectPosting={setSelectedPostingId}
      />

      <PostingsPreview posting={selectedPosting} loading={loading} />
    </div>
  );
}
