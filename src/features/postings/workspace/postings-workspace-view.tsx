import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import {
  createPostingItemViewModel,
  type PostingDetailLoadState,
} from "@/features/postings/view-model/posting-item-view-model";
import { usePostingsList } from "@/features/postings/workspace/postings-workspace-provider";

import { PostingsListPanel } from "@/features/postings/list/postings-list-panel";
import { PostingPreviewPanel } from "@/features/postings/preview/posting-preview-panel";

export function PostingsWorkspaceView() {
  const {
    activeQueue,
    listError,
    listLoading,
    loadPostingDetail,
    postings,
    refreshList,
  } = usePostingsList();
  const [selectedPostingId, setSelectedPostingId] = useState<number | null>(
    null,
  );
  const [detailState, setDetailState] = useState<PostingDetailLoadState>({
    status: "idle",
  });
  const detailRequestIdRef = useRef(0);

  const { activePostingItems, activePostingRows } = useMemo(() => {
    const items = postings.map(createPostingItemViewModel);

    return {
      activePostingItems: items,
      activePostingRows: items.map((posting) => posting.row),
    };
  }, [postings]);
  useEffect(() => {
    if (listLoading) return;

    if (!activePostingItems.length) {
      detailRequestIdRef.current += 1;
      setSelectedPostingId(null);
      setDetailState({ status: "idle" });
      return;
    }

    const selectedPostingIsVisible = activePostingItems.some(
      (posting) => posting.id === selectedPostingId,
    );

    if (!selectedPostingIsVisible) {
      detailRequestIdRef.current += 1;
      setSelectedPostingId(activePostingItems[0].id);
      setDetailState({ status: "idle" });
    }
  }, [activePostingItems, listLoading, selectedPostingId]);

  const handleSelectPosting = useCallback(
    (postingId: number) => {
      const requestId = detailRequestIdRef.current + 1;
      detailRequestIdRef.current = requestId;
      setSelectedPostingId(postingId);
      setDetailState({ status: "loading", postingId });

      void loadPostingDetail(postingId)
        .then((detail) => {
          if (detailRequestIdRef.current !== requestId) return;
          setDetailState({ status: "loaded", postingId, detail });
        })
        .catch((unknownError) => {
          if (detailRequestIdRef.current !== requestId) return;
          console.error("Failed to load job posting detail", unknownError);
          setDetailState({
            status: "failed",
            postingId,
            message:
              "Die Ausschreibung konnte gerade nicht geladen werden. Bitte versuche es erneut.",
          });
        });
    },
    [loadPostingDetail],
  );

  const selectedPosting = useMemo(
    () =>
      activePostingItems.find((posting) => posting.id === selectedPostingId) ??
      null,
    [activePostingItems, selectedPostingId],
  );

  return (
    <ResizablePanelGroup
      orientation="horizontal"
      className="h-full min-h-0 min-w-0 flex-1 overflow-hidden"
    >
      <ResizablePanel
        id="postings-list"
        defaultSize="35%"
        minSize="15%"
        maxSize="55%"
        className="h-full min-w-0"
      >
        <PostingsListPanel
          activeQueue={activeQueue}
          error={listError}
          loading={listLoading}
          postings={activePostingRows}
          selectedPostingId={selectedPostingId}
          onRetry={refreshList}
          onSelectPosting={handleSelectPosting}
        />
      </ResizablePanel>

      <ResizableHandle className="transition-colors hover:bg-border/80 active:bg-primary/20 before:pointer-events-none before:absolute before:left-1/2 before:top-1/2 before:z-10 before:h-6 before:w-1 before:-translate-x-1/2 before:-translate-y-1/2 before:rounded-full before:bg-muted-foreground/20 before:transition-all before:duration-200 hover:before:h-10 hover:before:bg-muted-foreground/40 active:before:h-16 active:before:bg-primary" />

      <ResizablePanel
        id="postings-preview"
        defaultSize="65%"
        minSize="45%"
        className="h-full min-w-0"
      >
        <PostingPreviewPanel
          detailState={detailState}
          posting={selectedPosting?.preview ?? null}
          loading={listLoading}
          onRetryDetail={
            selectedPostingId === null
              ? undefined
              : () => handleSelectPosting(selectedPostingId)
          }
        />
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
