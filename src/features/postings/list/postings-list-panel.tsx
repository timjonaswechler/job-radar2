import { AlertCircleIcon, ListFilter, InboxIcon, Search } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import {
  Alert,
  AlertAction,
  AlertDescription,
  AlertTitle,
} from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import type { PostingQueue } from "@/features/postings/queues/posting-queues";
import type { PostingListItemViewModel } from "@/features/postings/view-model/posting-item-view-model";
import type { JobPostingsLoadError } from "@/features/postings/workspace/postings-workspace-provider";
import { cn } from "@/lib/utils";

import { PostingListRow } from "./posting-list-row";

type PostingsListPanelProps = {
  activeQueue: PostingQueue;
  error: JobPostingsLoadError | null;
  loading: boolean;
  postings: PostingListItemViewModel[];
  selectedPostingId: number | null;
  onRetry: () => Promise<void>;
  onSelectPosting: (postingId: number) => void;
  className?: string;
};

export function PostingsListPanel({
  activeQueue,
  error,
  loading,
  postings,
  selectedPostingId,
  onRetry,
  onSelectPosting,
  className,
}: PostingsListPanelProps) {
  return (
    <section className={cn("flex h-full min-h-0 min-w-0 flex-col", className)}>
      <div className="flex shrink-0 flex-col gap-3 p-4 pb-3">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-2">
            <h1 className="truncate text-xl font-medium leading-none">
              {activeQueue.label}
            </h1>
            {loading ? (
              <Skeleton className="h-5 w-12" />
            ) : (
              <Badge variant="secondary" radius="full">
                {postings.length}
              </Badge>
            )}
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label="Filter folgen später"
            disabled
          >
            <ListFilter aria-hidden="true" />
          </Button>
        </div>

        <p className="text-xs text-muted-foreground">
          {activeQueue.description} Neue und gelesene Anzeigen werden wie in
          einem Postfach direkt in der Liste markiert.
        </p>

        <InputGroup className="h-7 w-full max-w-sm">
          <InputGroupInput
            className="h-7"
            placeholder="Suche folgt später…"
            disabled
          />
          <InputGroupAddon>
            <Search />
          </InputGroupAddon>
        </InputGroup>
      </div>

      <Separator />

      <div className="flex min-h-0 flex-1 flex-col py-2">
        {error ? (
          <PostingsListError error={error} onRetry={onRetry} />
        ) : loading ? (
          <PostingsListSkeleton />
        ) : postings.length ? (
          <ScrollArea className="h-full min-h-0 flex-1 overflow-hidden [&_[data-orientation=vertical][data-slot=scroll-area-scrollbar]]:w-1.5">
            <FlatPostingsList
              postings={postings}
              selectedPostingId={selectedPostingId}
              onSelectPosting={onSelectPosting}
            />
          </ScrollArea>
        ) : (
          <PostingsListEmpty queueLabel={activeQueue.label} />
        )}
      </div>
    </section>
  );
}

function FlatPostingsList({
  postings,
  selectedPostingId,
  onSelectPosting,
}: {
  postings: PostingListItemViewModel[];
  selectedPostingId: number | null;
  onSelectPosting: (postingId: number) => void;
}) {
  return (
    <div className="flex flex-col gap-1 px-2">
      {postings.map((posting) => (
        <PostingListRow
          key={posting.id}
          posting={posting}
          selected={posting.id === selectedPostingId}
          onSelectPosting={onSelectPosting}
        />
      ))}
    </div>
  );
}

function PostingsListError({
  error,
  onRetry,
}: {
  error: JobPostingsLoadError;
  onRetry: () => Promise<void>;
}) {
  return (
    <div className="flex min-h-0 flex-1 p-4">
      <Alert variant="destructive">
        <AlertCircleIcon aria-hidden="true" />
        <AlertTitle>{error.title}</AlertTitle>
        <AlertDescription>{error.description}</AlertDescription>
        <AlertAction>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => void onRetry()}
          >
            Erneut versuchen
          </Button>
        </AlertAction>
      </Alert>
    </div>
  );
}

function PostingsListEmpty({ queueLabel }: { queueLabel: string }) {
  return (
    <div className="flex min-h-full flex-1 p-4">
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <InboxIcon aria-hidden="true" />
          </EmptyMedia>
          <EmptyTitle>Keine Anzeigen in „{queueLabel}“</EmptyTitle>
          <EmptyDescription>
            Wenn neue Suchläufe Anzeigen persistieren, erscheinen sie in der
            passenden Queue. Diese Ansicht bleibt bewusst leer statt
            Backend-Zustände zu erfinden.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    </div>
  );
}

function PostingsListSkeleton() {
  return (
    <div className="flex flex-col gap-1 px-2">
      {Array.from({ length: 6 }).map((_, index) => (
        <div key={index} className="grid gap-2 rounded-lg px-2.5 py-2.5">
          <div className="flex items-start justify-between gap-3">
            <div className="grid flex-1 gap-2">
              <Skeleton className="h-4 w-2/3" />
              <Skeleton className="h-3 w-1/2" />
            </div>
            <Skeleton className="h-3 w-14" />
          </div>
          <div className="flex justify-end gap-2">
            <Skeleton className="h-5 w-14 rounded-full" />
            <Skeleton className="h-5 w-24 rounded-full" />
          </div>
          <Skeleton className="h-6 w-full rounded-md" />
        </div>
      ))}
    </div>
  );
}
