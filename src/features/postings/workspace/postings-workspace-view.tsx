import { useMemo } from "react";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { createPostingItemViewModel } from "@/features/postings/view-model/posting-item-view-model";
import { usePostingsList } from "@/features/postings/workspace/postings-workspace-provider";

import { PostingsListPanel } from "@/features/postings/list/postings-list-panel";
import { PostingPreviewPanel } from "@/features/postings/preview/posting-preview-panel";

export function PostingsWorkspaceView() {
  const {
    activeQueue,
    detailState,
    listError,
    listLoading,
    postings,
    refreshList,
    retryDetail,
    selectedPostingId,
    selectPosting,
  } = usePostingsList();

  const { activePostingItems, activePostingRows } = useMemo(() => {
    const items = postings.map(createPostingItemViewModel);
    return {
      activePostingItems: items,
      activePostingRows: items.map((posting) => posting.row),
    };
  }, [postings]);

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
          onSelectPosting={selectPosting}
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
          onRetryDetail={selectedPostingId === null ? undefined : retryDetail}
        />
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
