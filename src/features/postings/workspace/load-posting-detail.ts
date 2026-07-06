import type {
  JobPosting,
  JobPostingDetail,
  JobPostingQueueId,
} from "@/lib/api/job-postings";

export type PostingDetailLoader = (postingId: number) => Promise<JobPostingDetail>;
export type PostingListUpdater = (updater: (current: JobPosting[]) => JobPosting[]) => void;

export type LoadPostingDetailForWorkspaceInput = {
  activeQueueId: JobPostingQueueId;
  currentPostings: JobPosting[];
  postingId: number;
  getPostingDetail: PostingDetailLoader;
  setPostings: PostingListUpdater;
  refreshCounts: () => Promise<void> | void;
};

export async function loadPostingDetailForWorkspace({
  activeQueueId,
  currentPostings,
  postingId,
  getPostingDetail,
  setPostings,
  refreshCounts,
}: LoadPostingDetailForWorkspaceInput): Promise<JobPostingDetail> {
  const postingBeforeLoad = currentPostings.find(
    (posting) => posting.id === postingId,
  );
  const detail = await getPostingDetail(postingId);

  setPostings((current) =>
    current.map((posting) => (posting.id === postingId ? detail : posting)),
  );

  if (
    activeQueueId === "inbox" &&
    postingBeforeLoad?.readState === "unread" &&
    detail.readState === "read"
  ) {
    await refreshCounts();
  }

  return detail;
}
