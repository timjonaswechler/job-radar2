import { memo } from "react";

import { Building2, MapPin } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { PostingPreparationStepper } from "@/features/postings/list/posting-preparation-stepper";
import type { PostingListItemViewModel } from "@/features/postings/view-model/posting-item-view-model";
import { cn } from "@/lib/utils";

type PostingListRowProps = {
  posting: PostingListItemViewModel;
  selected: boolean;
  onSelectPosting: (postingId: number) => void;
};

export const PostingListRow = memo(function PostingListRow({
  posting,
  selected,
  onSelectPosting,
}: PostingListRowProps) {
  return (
    <button
      type="button"
      aria-current={selected ? "true" : undefined}
      aria-label={`${posting.title}, ${posting.company}`}
      className={cn(
        "w-full overflow-hidden rounded-lg px-2.5 py-2.5 text-left ring-inset transition-colors [contain-intrinsic-size:7rem] [content-visibility:auto] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/30",
        selected ? "bg-muted ring-1 ring-border" : "hover:bg-muted/75",
      )}
      onClick={(event) => {
        event.currentTarget.blur();
        onSelectPosting(posting.id);
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
          {posting.preparationProgress ? (
            <PostingPreparationStepper progress={posting.preparationProgress} />
          ) : posting.processSlotLabel ? (
            <div className="mt-2 flex h-6 min-w-0 items-center rounded-md border border-dashed bg-background/60 px-2 text-xs leading-none text-muted-foreground">
              <span className="truncate">{posting.processSlotLabel}</span>
            </div>
          ) : null}
        </div>
      </div>
    </button>
  );
});
