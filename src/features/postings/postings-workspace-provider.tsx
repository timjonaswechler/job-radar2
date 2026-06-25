import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";

import {
  getJobPostingQueueCounts,
  listJobPostingsForQueue,
  updateJobPostingState,
  type JobPosting,
} from "@/lib/api/job-postings";
import {
  EMPTY_QUEUE_COUNTS,
  getPostingQueueIdFromPath,
  getQueueDefinition,
  type PostingQueue,
  type PostingQueueId,
  type QueueCounts,
} from "@/features/postings/posting-workflow";

export type JobPostingsLoadError = {
  title: string;
  description: string;
};

type PostingsWorkspaceContextValue = {
  activeQueue: PostingQueue;
  activeQueueId: PostingQueueId;
  counts: QueueCounts;
  countsError: JobPostingsLoadError | null;
  countsLoading: boolean;
  listError: JobPostingsLoadError | null;
  listLoading: boolean;
  postings: JobPosting[];
  markPostingAsRead: (postingId: number) => Promise<void>;
  refreshCounts: () => Promise<void>;
  refreshList: () => Promise<void>;
  refreshWorkspace: () => Promise<void>;
};

type PostingsWorkspaceProviderProps = {
  children: ReactNode;
  pathname: string;
};

const PostingsWorkspaceContext =
  createContext<PostingsWorkspaceContextValue | null>(null);

const countsLoadError = {
  title: "Queue-Zahlen konnten nicht geladen werden",
  description:
    "Die Zahlen in der Stellenanzeigen-Navigation sind gerade nicht erreichbar. Die Listenansicht kann trotzdem separat geladen werden.",
} satisfies JobPostingsLoadError;

const listLoadError = {
  title: "Stellenanzeigen konnten nicht geladen werden",
  description:
    "Die gespeicherten Anzeigen sind gerade nicht erreichbar. Prüfe, ob die lokale App-Datenbank verfügbar ist, und versuche es erneut.",
} satisfies JobPostingsLoadError;

export function PostingsWorkspaceProvider({
  children,
  pathname,
}: PostingsWorkspaceProviderProps) {
  const activeQueueId = getPostingQueueIdFromPath(pathname);
  const activeQueue = getQueueDefinition(activeQueueId);
  const shouldLoadPostings =
    pathname === "/postings" || pathname.startsWith("/postings/");

  const [counts, setCounts] = useState<QueueCounts>(EMPTY_QUEUE_COUNTS);
  const [countsLoading, setCountsLoading] = useState(true);
  const [countsError, setCountsError] = useState<JobPostingsLoadError | null>(
    null,
  );
  const [postings, setPostings] = useState<JobPosting[]>([]);
  const [listLoading, setListLoading] = useState(false);
  const [listError, setListError] = useState<JobPostingsLoadError | null>(null);
  const pendingReadPostingIds = useRef(new Set<number>());

  const refreshCounts = useCallback(async () => {
    try {
      setCountsLoading(true);
      setCountsError(null);
      setCounts(await getJobPostingQueueCounts());
    } catch (unknownError) {
      console.error("Failed to load job posting queue counts", unknownError);
      setCounts(EMPTY_QUEUE_COUNTS);
      setCountsError(countsLoadError);
    } finally {
      setCountsLoading(false);
    }
  }, []);

  const refreshList = useCallback(async () => {
    if (!shouldLoadPostings) {
      setPostings([]);
      setListLoading(false);
      setListError(null);
      return;
    }

    try {
      setListLoading(true);
      setListError(null);
      setPostings(await listJobPostingsForQueue(activeQueueId));
    } catch (unknownError) {
      console.error("Failed to load job postings", unknownError);
      setPostings([]);
      setListError(listLoadError);
    } finally {
      setListLoading(false);
    }
  }, [activeQueueId, shouldLoadPostings]);

  const refreshWorkspace = useCallback(async () => {
    await Promise.all([refreshCounts(), refreshList()]);
  }, [refreshCounts, refreshList]);

  const markPostingAsRead = useCallback(
    async (postingId: number) => {
      const posting = postings.find((item) => item.id === postingId);

      if (
        activeQueueId !== "inbox" ||
        !posting ||
        posting.readState === "read" ||
        pendingReadPostingIds.current.has(postingId)
      ) {
        return;
      }

      pendingReadPostingIds.current.add(postingId);
      setPostings((currentPostings) =>
        currentPostings.map((item) =>
          item.id === postingId ? { ...item, readState: "read" } : item,
        ),
      );
      setCounts((currentCounts) => ({
        ...currentCounts,
        newInbox: Math.max(0, currentCounts.newInbox - 1),
        reviewInbox: currentCounts.reviewInbox + 1,
      }));

      try {
        const updatedPosting = await updateJobPostingState(postingId, {
          readState: "read",
        });

        setPostings((currentPostings) =>
          currentPostings.map((item) =>
            item.id === postingId ? updatedPosting : item,
          ),
        );
      } catch (unknownError) {
        console.error("Failed to mark job posting as read", unknownError);
        setPostings((currentPostings) =>
          currentPostings.map((item) =>
            item.id === postingId ? { ...item, readState: "unread" } : item,
          ),
        );
        toast.error("Anzeige konnte nicht als gelesen markiert werden.", {
          description:
            "Der Neu-Status bleibt erhalten. Bitte versuche es gleich noch einmal.",
        });
      } finally {
        pendingReadPostingIds.current.delete(postingId);
        void refreshCounts();
      }
    },
    [activeQueueId, postings, refreshCounts],
  );

  useEffect(() => {
    void refreshCounts();
  }, [refreshCounts]);

  useEffect(() => {
    void refreshList();
  }, [refreshList]);

  const value = useMemo(
    () => ({
      activeQueue,
      activeQueueId,
      counts,
      countsError,
      countsLoading,
      listError,
      listLoading,
      postings,
      markPostingAsRead,
      refreshCounts,
      refreshList,
      refreshWorkspace,
    }),
    [
      activeQueue,
      activeQueueId,
      counts,
      countsError,
      countsLoading,
      listError,
      listLoading,
      postings,
      markPostingAsRead,
      refreshCounts,
      refreshList,
      refreshWorkspace,
    ],
  );

  return (
    <PostingsWorkspaceContext.Provider value={value}>
      {children}
    </PostingsWorkspaceContext.Provider>
  );
}

export function usePostingsWorkspace() {
  const context = useContext(PostingsWorkspaceContext);

  if (!context) {
    throw new Error(
      "usePostingsWorkspace must be used within PostingsWorkspaceProvider.",
    );
  }

  return context;
}
