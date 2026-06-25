import {
  AlertCircleIcon,
  Building2Icon,
  FilterIcon,
  InboxIcon,
  MapPinIcon,
  Search,
} from "lucide-react";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group";
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
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import type { JobPosting } from "@/lib/api/job-postings";
import { cn } from "@/lib/utils";
import {
  formatAbsoluteDate,
  formatLocations,
  formatRelativeDate,
  getSourceLabel,
  getWorkflowBadge,
  INBOX_ANCHORS,
  type PostingInboxAnchorId,
  type PostingQueue,
} from "@/features/postings/postings-view-model";

type PostingsListProps = {
  activeInboxAnchorId: PostingInboxAnchorId | null;
  activeQueue: PostingQueue;
  error: string | null;
  loading: boolean;
  postings: JobPosting[];
  selectedPostingId: number | null;
  onRetry: () => Promise<void>;
  onSelectPosting: (postingId: number) => void;
  className?: string;
};

export function PostingsList({
  activeInboxAnchorId,
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
    <section className={cn("flex flex-col gap-3 py-3", className)}>
      <div className="flex items-center justify-between gap-4 px-2 py-0.5">
        <div className="flex  items-center gap-2">
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
          <FilterIcon aria-hidden="true" />
        </Button>
      </div>

      <p className="px-2 text-xs text-muted-foreground">
        {activeQueue.description}
        {activeInboxAnchorId ? (
          <span>
            {" "}
            Prioritätsanker: {getInboxAnchorLabel(activeInboxAnchorId)} — die
            Inbox-Liste bleibt vollständig.
          </span>
        ) : null}
      </p>

      <Separator />
      <div className="px-2">
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

      <div className="flex flex-1 flex-col">
        {error ? (
          <PostingsListError error={error} onRetry={onRetry} />
        ) : loading ? (
          <PostingsListSkeleton />
        ) : postings.length ? (
          <ScrollArea className="h-full min-h-0 flex-1 overflow-hidden [&_[data-orientation=vertical][data-slot=scroll-area-scrollbar]]:w-1.5">
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
          </ScrollArea>
        ) : (
          <PostingsListEmpty queueLabel={activeQueue.label} />
        )}
      </div>
    </section>
  );
}

function getInboxAnchorLabel(anchorId: PostingInboxAnchorId) {
  return (
    INBOX_ANCHORS.find((anchor) => anchor.id === anchorId)?.label ?? "Inbox"
  );
}

function PostingsListError({
  error,
  onRetry,
}: {
  error: string;
  onRetry: () => Promise<void>;
}) {
  return (
    <div className="p-4">
      <Alert variant="destructive">
        <AlertCircleIcon aria-hidden="true" />
        <AlertTitle>Stellenanzeigen konnten nicht geladen werden</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
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
    <div className="flex min-h-80 p-4">
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
  posting: JobPosting;
  selected: boolean;
  onSelect: () => void;
}) {
  const workflowBadge = getWorkflowBadge(posting);
  const sourceLabel = getSourceLabel(posting);
  const locationLabel = formatLocations(posting.locations);
  const relativeLastSeen = formatRelativeDate(posting.lastSeenAt);
  const absoluteLastSeen = formatAbsoluteDate(posting.lastSeenAt);

  return (
    <button
      type="button"
      aria-current={selected ? "true" : undefined}
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
            posting.readState === "unread"
              ? "bg-primary"
              : "bg-muted-foreground/30",
          )}
        />

        <div className="w-0 flex-1 overflow-hidden">
          <div className="flex w-full items-center justify-between gap-2">
            <div className="truncate text-sm font-medium leading-5">
              {posting.title}
            </div>
            <time
              className="text-nowrap text-xs leading-5 text-muted-foreground"
              dateTime={posting.lastSeenAt}
              title={absoluteLastSeen}
            >
              {relativeLastSeen}
            </time>
          </div>

          <div className="flex min-w-0 items-end gap-2">
            <div className="w-0 flex-1 overflow-hidden">
              <div className="flex min-w-0 items-center gap-2 text-xs leading-4 text-foreground/90">
                <Building2Icon aria-hidden="true" className="size-3" />
                <span className="truncate font-medium">{posting.company}</span>
              </div>
              <div className="flex min-w-0 items-center gap-2 text-xs leading-4 text-muted-foreground">
                <MapPinIcon aria-hidden="true" className="size-3" />
                <span className="truncate">{locationLabel}</span>
              </div>
            </div>

            <div className="flex max-w-40 flex-col items-end gap-1">
              <div className="flex flex-wrap justify-end gap-1">
                <Badge
                  variant={
                    posting.readState === "unread"
                      ? "primary-light"
                      : "secondary"
                  }
                  size="sm"
                  radius="full"
                >
                  {posting.readState === "unread" ? "Neu" : "Gelesen"}
                </Badge>
                <Badge variant={workflowBadge.variant} size="sm" radius="full">
                  {workflowBadge.label}
                </Badge>
              </div>
              <span className="max-w-full truncate text-xs text-muted-foreground">
                {sourceLabel}
              </span>
            </div>
          </div>
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
        </div>
      ))}
    </div>
  );
}
