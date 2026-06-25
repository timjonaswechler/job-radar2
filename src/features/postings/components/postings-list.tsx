import {
  AlertCircleIcon,
  ListFilter,
  InboxIcon,
  Search,
  MapPin,
  Building2,
} from "lucide-react";

import { Badge } from "@/components/reui/badge";
import {
  Alert,
  AlertAction,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
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
import type {
  PostingListItemViewModel,
  PostingQueue,
} from "@/features/postings/postings-view-model";
import type { JobPostingsLoadError } from "@/features/postings/postings-workspace-provider";
import { cn } from "@/lib/utils";

type PostingsListProps = {
  activeQueue: PostingQueue;
  error: JobPostingsLoadError | null;
  loading: boolean;
  postings: PostingListItemViewModel[];
  selectedPostingId: number | null;
  onRetry: () => Promise<void>;
  onSelectPosting: (postingId: number) => void;
  className?: string;
};

export function PostingsList({
  activeQueue,
  error,
  loading,
  postings,
  selectedPostingId,
  onRetry,
  onSelectPosting,
  className,
}: PostingsListProps) {
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
          onSelect={() => onSelectPosting(posting.id)}
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

function PostingListRow({
  posting,
  selected,
  onSelect,
}: {
  posting: PostingListItemViewModel;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      aria-current={selected ? "true" : undefined}
      aria-label={`${posting.title}, ${posting.company}`}
      className={cn(
        "w-full overflow-hidden rounded-lg px-2.5 py-2.5 text-left ring-inset transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/30",
        selected ? "bg-muted ring-1 ring-border" : "hover:bg-muted/75",
      )}
      onClick={(event) => {
        event.currentTarget.blur();
        onSelect();
      }}
    >
      <div className="flex min-w-0 items-start gap-2.5">
        <span
          aria-hidden="true"
          className={cn(
            "mt-2 size-2 shrink-0 rounded-full",
            posting.isUnread ? "bg-primary" : "bg-muted-foreground/30",
          )}
        />

        <div className="w-0 flex-1 overflow-hidden">
          <div className="flex w-full items-center justify-between gap-2">
            <div className="flex min-w-0 items-center gap-1.5 text-sm font-medium leading-5">
              <span className="truncate">{posting.title}</span>
            </div>
            <time
              className="shrink-0 text-nowrap text-xs leading-5 text-muted-foreground"
              dateTime={posting.lastActivityDateTime}
              title={posting.lastActivityTitle}
            >
              {posting.lastActivityLabel}
            </time>
          </div>
          <div className="mt-1 flex min-w-0 items-center gap-2">
            <div className="flex min-w-0 flex-1 items-center gap-1.5 text-xs leading-4 text-muted-foreground">
              <Building2 aria-hidden="true" className="size-3.5 shrink-0" />
              <span className="truncate text-muted-foreground">
                {posting.company}
              </span>
            </div>
            <div className="flex shrink-0 flex-wrap justify-end gap-1">
              <Badge
                variant={posting.readStateBadge.variant}
                size="sm"
                radius="full"
              >
                {posting.readStateBadge.label}
              </Badge>
              <Badge
                variant={posting.interestStateBadge.variant}
                size="sm"
                radius="full"
              >
                {posting.interestStateBadge.label}
              </Badge>
            </div>
          </div>
          <div className="mt-1 flex min-w-0 items-center gap-2">
            <div className="flex min-w-0 flex-1 items-center gap-1.5 text-xs leading-4 text-muted-foreground">
              <MapPin aria-hidden="true" className="size-3.5 shrink-0" />
              <span className="truncate">{posting.locationLabel}</span>
            </div>
          </div>
          {posting.processSlotLabel ? (
            <div className="mt-2 flex h-6 min-w-0 items-center rounded-md border border-dashed bg-background/60 px-2 text-xs leading-none text-muted-foreground">
              <span className="truncate">{posting.processSlotLabel}</span>
            </div>
          ) : null}
        </div>
      </div>
    </button>
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
