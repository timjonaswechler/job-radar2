import { useEffect, useMemo, useState } from "react";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { createPostingItemViewModel } from "@/features/postings/postings-view-model";
import { usePostingsWorkspace } from "@/features/postings/postings-workspace-provider";

import { PostingsList } from "./postings-list";
import { PostingsPreview } from "./postings-preview";

export function Postings() {
  const { activeQueue, listError, listLoading, postings, refreshList } =
    usePostingsWorkspace();
  const [selectedPostingId, setSelectedPostingId] = useState<number | null>(
    null,
  );

  const activePostingItems = useMemo(
    () => postings.map(createPostingItemViewModel),
    [postings],
  );
  const activePostingRows = useMemo(
    () => activePostingItems.map((posting) => posting.row),
    [activePostingItems],
  );
  useEffect(() => {
    if (listLoading) return;

    if (!activePostingItems.length) {
      setSelectedPostingId(null);
      return;
    }

    const selectedPostingIsVisible = activePostingItems.some(
      (posting) => posting.id === selectedPostingId,
    );

    if (!selectedPostingIsVisible) {
      setSelectedPostingId(activePostingItems[0].id);
    }
  }, [activePostingItems, listLoading, selectedPostingId]);

  const selectedPosting =
    activePostingItems.find((posting) => posting.id === selectedPostingId) ??
    null;

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
        <PostingsList
          activeQueue={activeQueue}
          error={listError}
          loading={listLoading}
          postings={activePostingRows}
          selectedPostingId={selectedPostingId}
          onRetry={refreshList}
          onSelectPosting={setSelectedPostingId}
        />
      </ResizablePanel>

      <ResizableHandle className="transition-colors hover:bg-border/80 active:bg-primary/20 before:pointer-events-none before:absolute before:left-1/2 before:top-1/2 before:z-10 before:h-6 before:w-1 before:-translate-x-1/2 before:-translate-y-1/2 before:rounded-full before:bg-muted-foreground/20 before:transition-all before:duration-200 hover:before:h-10 hover:before:bg-muted-foreground/40 active:before:h-16 active:before:bg-primary" />

      <ResizablePanel
        id="postings-preview"
        defaultSize="65%"
        minSize="45%"
        className="h-full min-w-0"
      >
        <PostingsPreview
          posting={selectedPosting?.preview ?? null}
          loading={listLoading}
        />
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
