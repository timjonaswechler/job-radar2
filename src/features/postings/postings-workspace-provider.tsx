import {
  createContext,
  type ReactNode,
  type SetStateAction,
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
  getPostingDetail,
  listJobPostingsForQueue,
  updateJobPostingState,
  type JobPosting,
  type JobPostingDetail,
} from "@/lib/api/job-postings";
import { loadPostingDetailForWorkspace } from "@/features/postings/posting-detail-workspace";
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

type PostingsCountsContextValue = {
  counts: QueueCounts;
  countsError: JobPostingsLoadError | null;
  countsLoading: boolean;
  refreshCounts: () => Promise<void>;
};

type PostingsListContextValue = {
  activeQueue: PostingQueue;
  activeQueueId: PostingQueueId;
  listError: JobPostingsLoadError | null;
  listLoading: boolean;
  postings: JobPosting[];
  loadPostingDetail: (postingId: number) => Promise<JobPostingDetail>;
  markPostingAsRead: (postingId: number) => Promise<void>;
  refreshList: () => Promise<void>;
  refreshWorkspace: () => Promise<void>;
};

type PostingsWorkspaceContextValue = PostingsCountsContextValue &
  PostingsListContextValue;

type PostingsWorkspaceProviderProps = {
  children: ReactNode;
  pathname: string;
};

const PostingsCountsContext =
  createContext<PostingsCountsContextValue | null>(null);
const PostingsListContext = createContext<PostingsListContextValue | null>(
  null,
);

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
  const postingsRef = useRef<JobPosting[]>([]);
  const [listLoading, setListLoading] = useState(false);
  const [listError, setListError] = useState<JobPostingsLoadError | null>(null);
  const pendingReadPostingIds = useRef(new Set<number>());

  const setPostingsState = useCallback(
    (nextPostings: SetStateAction<JobPosting[]>) => {
      setPostings((currentPostings) => {
        const resolvedPostings =
          typeof nextPostings === "function"
            ? nextPostings(currentPostings)
            : nextPostings;

        postingsRef.current = resolvedPostings;
        return resolvedPostings;
      });
    },
    [],
  );

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
      setPostingsState([]);
      setListLoading(false);
      setListError(null);
      return;
    }

    try {
      setListLoading(true);
      setListError(null);
      setPostingsState(await listJobPostingsForQueue(activeQueueId));
    } catch (unknownError) {
      console.error("Failed to load job postings", unknownError);
      setPostingsState([]);
      setListError(listLoadError);
    } finally {
      setListLoading(false);
    }
  }, [activeQueueId, setPostingsState, shouldLoadPostings]);

  const refreshWorkspace = useCallback(async () => {
    await Promise.all([refreshCounts(), refreshList()]);
  }, [refreshCounts, refreshList]);

  const loadPostingDetail = useCallback(
    async (postingId: number) =>
      loadPostingDetailForWorkspace({
        activeQueueId,
        currentPostings: postingsRef.current,
        postingId,
        getPostingDetail,
        setPostings: setPostingsState,
        refreshCounts,
      }),
    [activeQueueId, refreshCounts, setPostingsState],
  );

  const markPostingAsRead = useCallback(
    async (postingId: number) => {
      const posting = postingsRef.current.find((item) => item.id === postingId);

      if (
        activeQueueId !== "inbox" ||
        !posting ||
        posting.readState === "read" ||
        pendingReadPostingIds.current.has(postingId)
      ) {
        return;
      }

      pendingReadPostingIds.current.add(postingId);
      setPostingsState((currentPostings) =>
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

        setPostingsState((currentPostings) =>
          currentPostings.map((item) =>
            item.id === postingId ? updatedPosting : item,
          ),
        );
      } catch (unknownError) {
        console.error("Failed to mark job posting as read", unknownError);
        setPostingsState((currentPostings) =>
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
    [activeQueueId, refreshCounts, setPostingsState],
  );

  useEffect(() => {
    void refreshCounts();
  }, [refreshCounts]);

  useEffect(() => {
    void refreshList();
  }, [refreshList]);

  const countsValue = useMemo(
    () => ({
      counts,
      countsError,
      countsLoading,
      refreshCounts,
    }),
    [counts, countsError, countsLoading, refreshCounts],
  );

  const listValue = useMemo(
    () => ({
      activeQueue,
      activeQueueId,
      listError,
      listLoading,
      postings,
      loadPostingDetail,
      markPostingAsRead,
      refreshList,
      refreshWorkspace,
    }),
    [
      activeQueue,
      activeQueueId,
      listError,
      listLoading,
      postings,
      loadPostingDetail,
      markPostingAsRead,
      refreshList,
      refreshWorkspace,
    ],
  );

  return (
    <PostingsCountsContext.Provider value={countsValue}>
      <PostingsListContext.Provider value={listValue}>
        {children}
      </PostingsListContext.Provider>
    </PostingsCountsContext.Provider>
  );
}

export function usePostingsCounts() {
  const context = useContext(PostingsCountsContext);

  if (!context) {
    throw new Error(
      "usePostingsCounts must be used within PostingsWorkspaceProvider.",
    );
  }

  return context;
}

export function usePostingsList() {
  const context = useContext(PostingsListContext);

  if (!context) {
    throw new Error(
      "usePostingsList must be used within PostingsWorkspaceProvider.",
    );
  }

  return context;
}

export function usePostingsWorkspace(): PostingsWorkspaceContextValue {
  return {
    ...usePostingsCounts(),
    ...usePostingsList(),
  };
}
