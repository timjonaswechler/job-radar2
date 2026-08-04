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

import {
  getJobPostingQueueCounts,
  getPostingDetail,
  listJobPostingsForQueue,
  PostingTransportError,
  type JobPosting,
  type JobPostingDetail,
} from "@/lib/api/job-postings";
import {
  EMPTY_QUEUE_COUNTS,
  getPostingQueueIdFromPath,
  getQueueDefinition,
  type PostingQueue,
  type PostingQueueId,
  type QueueCounts,
} from "@/features/postings/queues/posting-queues";
import type { PostingDetailLoadState } from "@/features/postings/view-model/posting-item-view-model";

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
  detailState: PostingDetailLoadState;
  listError: JobPostingsLoadError | null;
  listLoading: boolean;
  postings: JobPosting[];
  refreshList: () => Promise<void>;
  retryDetail: () => void;
  selectedPostingId: number | null;
  selectPosting: (postingId: number) => void;
};

type PostingsWorkspaceProviderProps = {
  children: ReactNode;
  pathname: string;
};

const PostingsCountsContext = createContext<PostingsCountsContextValue | null>(null);
const PostingsListContext = createContext<PostingsListContextValue | null>(null);

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

const detailLoadMessage =
  "Die Ausschreibung konnte gerade nicht geladen werden. Bitte versuche es erneut.";

export function PostingsWorkspaceProvider({
  children,
  pathname,
}: PostingsWorkspaceProviderProps) {
  const activeQueueId = getPostingQueueIdFromPath(pathname);
  const activeQueue = getQueueDefinition(activeQueueId);
  const shouldLoadPostings =
    pathname === "/postings" || pathname.startsWith("/postings/");

  const mountedRef = useRef(true);
  const activeQueueIdRef = useRef(activeQueueId);
  const shouldLoadPostingsRef = useRef(shouldLoadPostings);
  activeQueueIdRef.current = activeQueueId;
  shouldLoadPostingsRef.current = shouldLoadPostings;

  const [counts, setCounts] = useState<QueueCounts>(EMPTY_QUEUE_COUNTS);
  const [countsLoading, setCountsLoading] = useState(true);
  const [countsError, setCountsError] = useState<JobPostingsLoadError | null>(null);
  const countsGenerationRef = useRef(0);

  const [postings, setPostings] = useState<JobPosting[]>([]);
  const postingsRef = useRef<JobPosting[]>([]);
  const [listLoading, setListLoading] = useState(false);
  const [listError, setListError] = useState<JobPostingsLoadError | null>(null);
  const listGenerationRef = useRef(0);

  const [selectedPostingId, setSelectedPostingId] = useState<number | null>(null);
  const [detailState, setDetailState] = useState<PostingDetailLoadState>({ status: "idle" });
  const detailGenerationRef = useRef(0);
  const detailCacheRef = useRef(new Map<number, JobPostingDetail>());

  const setPostingsState = useCallback((next: SetStateAction<JobPosting[]>) => {
    setPostings((current) => {
      const resolved = typeof next === "function" ? next(current) : next;
      postingsRef.current = resolved;
      return resolved;
    });
  }, []);

  const invalidateDetail = useCallback(() => {
    detailGenerationRef.current += 1;
    detailCacheRef.current.clear();
    setSelectedPostingId(null);
    setDetailState({ status: "idle" });
  }, []);

  const operationIsCurrent = useCallback(
    (generationRef: { current: number }, generation: number, queueId?: PostingQueueId) =>
      mountedRef.current &&
      generationRef.current === generation &&
      (queueId === undefined ||
        (activeQueueIdRef.current === queueId && shouldLoadPostingsRef.current)),
    [],
  );

  const refreshCounts = useCallback(async () => {
    const generation = ++countsGenerationRef.current;
    setCountsLoading(true);
    setCountsError(null);
    try {
      const nextCounts = await getJobPostingQueueCounts();
      if (operationIsCurrent(countsGenerationRef, generation)) setCounts(nextCounts);
    } catch (unknownError) {
      if (!operationIsCurrent(countsGenerationRef, generation)) return;
      console.error("Failed to load job posting queue counts", unknownError);
      setCounts(EMPTY_QUEUE_COUNTS);
      setCountsError(countsLoadError);
    } finally {
      if (operationIsCurrent(countsGenerationRef, generation)) setCountsLoading(false);
    }
  }, [operationIsCurrent]);

  const loadList = useCallback(
    async (queueId: PostingQueueId, resetDetail: boolean) => {
      const generation = ++listGenerationRef.current;
      if (resetDetail) invalidateDetail();
      if (!shouldLoadPostingsRef.current) {
        setPostingsState([]);
        setListLoading(false);
        setListError(null);
        return;
      }
      setListLoading(true);
      setListError(null);
      try {
        const nextPostings = await listJobPostingsForQueue(queueId);
        if (!operationIsCurrent(listGenerationRef, generation, queueId)) return;
        setPostingsState(nextPostings);
        if (resetDetail) setSelectedPostingId(nextPostings[0]?.id ?? null);
      } catch (unknownError) {
        if (!operationIsCurrent(listGenerationRef, generation, queueId)) return;
        console.error("Failed to load job postings", unknownError);
        setPostingsState([]);
        setListError(listLoadError);
      } finally {
        if (operationIsCurrent(listGenerationRef, generation, queueId)) {
          setListLoading(false);
        }
      }
    },
    [invalidateDetail, operationIsCurrent, setPostingsState],
  );

  const refreshList = useCallback(
    () => loadList(activeQueueId, true),
    [activeQueueId, loadList, shouldLoadPostings],
  );

  const startDetailLoad = useCallback(
    (postingId: number, force: boolean) => {
      const postingBeforeLoad = postingsRef.current.find((posting) => posting.id === postingId);
      if (!postingBeforeLoad) return;

      setSelectedPostingId(postingId);
      const cached = detailCacheRef.current.get(postingId);
      if (cached && !force) {
        setDetailState({ status: "loaded", postingId, detail: cached });
        return;
      }
      if (!force && detailState.status === "loading" && detailState.postingId === postingId) {
        return;
      }

      const generation = detailGenerationRef.current + 1;
      const queueId = activeQueueIdRef.current;
      detailGenerationRef.current = generation;
      setDetailState({ status: "loading", postingId });

      void getPostingDetail(postingId)
        .then(async (detail) => {
          if (!operationIsCurrent(detailGenerationRef, generation, queueId)) return;
          detailCacheRef.current.set(postingId, detail);
          setPostingsState((current) =>
            current.map((posting) => (posting.id === postingId ? detail : posting)),
          );
          setDetailState({ status: "loaded", postingId, detail });
          if (postingBeforeLoad.readState === "unread" && detail.readState === "read") {
            await refreshCounts();
          }
        })
        .catch((unknownError) => {
          if (!operationIsCurrent(detailGenerationRef, generation, queueId)) return;
          console.error("Failed to load job posting detail", unknownError);
          setDetailState({ status: "failed", postingId, message: detailLoadMessage });
          if (unknownError instanceof PostingTransportError && unknownError.kind === "after_read") {
            void refreshCounts();
            void loadList(queueId, false);
          }
        });
    },
    [detailState, loadList, operationIsCurrent, refreshCounts, setPostingsState],
  );

  const selectPosting = useCallback(
    (postingId: number) => startDetailLoad(postingId, false),
    [startDetailLoad],
  );

  const retryDetail = useCallback(() => {
    if (selectedPostingId !== null) startDetailLoad(selectedPostingId, true);
  }, [selectedPostingId, startDetailLoad]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      countsGenerationRef.current += 1;
      listGenerationRef.current += 1;
      detailGenerationRef.current += 1;
      detailCacheRef.current.clear();
    };
  }, []);

  useEffect(() => {
    void refreshCounts();
  }, [refreshCounts]);

  useEffect(() => {
    void refreshList();
  }, [refreshList]);

  const countsValue = useMemo(
    () => ({ counts, countsError, countsLoading, refreshCounts }),
    [counts, countsError, countsLoading, refreshCounts],
  );

  const listValue = useMemo(
    () => ({
      activeQueue,
      activeQueueId,
      detailState,
      listError,
      listLoading,
      postings,
      refreshList,
      retryDetail,
      selectedPostingId,
      selectPosting,
    }),
    [
      activeQueue,
      activeQueueId,
      detailState,
      listError,
      listLoading,
      postings,
      refreshList,
      retryDetail,
      selectedPostingId,
      selectPosting,
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
    throw new Error("usePostingsCounts must be used within PostingsWorkspaceProvider.");
  }
  return context;
}

export function usePostingsList() {
  const context = useContext(PostingsListContext);
  if (!context) {
    throw new Error("usePostingsList must be used within PostingsWorkspaceProvider.");
  }
  return context;
}
