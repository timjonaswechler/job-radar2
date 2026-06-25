import {
  Clock3Icon,
  ExternalLinkIcon,
  FilePenLineIcon,
  PanelRightIcon,
} from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import type { PostingPreviewViewModel } from "@/features/postings/postings-view-model";

type PostingsPreviewProps = {
  loading: boolean;
  posting: PostingPreviewViewModel | null;
};

const previewPanelClassName =
  "flex h-full min-h-0 min-w-0 flex-col gap-4 overflow-y-auto p-4";

export function PostingsPreview({ loading, posting }: PostingsPreviewProps) {
  if (loading) return <PreviewSkeleton />;

  if (!posting) {
    return (
      <aside className={previewPanelClassName}>
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <PanelRightIcon aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>Keine Anzeige ausgewählt</EmptyTitle>
            <EmptyDescription>
              Wähle links eine Queue und in der Mitte eine Anzeige aus. Bei
              leeren Queues bleibt das Detailpanel als ruhiger Platzhalter
              sichtbar.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      </aside>
    );
  }

  return (
    <aside className={previewPanelClassName}>
      <div className="flex items-start gap-3">
        <div className="flex size-10 shrink-0 items-center justify-center text-sm font-medium">
          {posting.companyInitials}
        </div>

        <div className="min-w-0 flex-1">
          <div className="truncate font-medium leading-5">{posting.title}</div>
          <div className="truncate text-xs text-muted-foreground">
            {posting.subtitle}
          </div>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        {posting.badges.map((badge) => (
          <Badge key={badge.label} variant={badge.variant} radius="full">
            {badge.label}
          </Badge>
        ))}
      </div>

      <Separator />

      <div className="flex flex-col gap-3 text-sm">
        {posting.detailRows.map((row) => (
          <PreviewDetailRow
            key={row.label}
            label={row.label}
            value={row.value}
          />
        ))}
      </div>

      <Separator />

      <div className="flex flex-col gap-3 text-sm">
        <div className="flex items-start gap-2 rounded-md border border-dashed p-3">
          <Clock3Icon
            aria-hidden="true"
            className="mt-0.5 size-4 shrink-0 text-muted-foreground"
          />
          <div className="grid gap-1">
            <div className="font-medium">Detaildaten folgen später</div>
            <p className="text-muted-foreground">
              Dieser Slice zeigt gespeicherte Stellenanzeigen in der
              Mailbox-Liste. Bewerbungsaktionen, Stepper und ausgebaute
              Detailinhalte bleiben bewusst separaten Issues vorbehalten.
            </p>
          </div>
        </div>
      </div>

      <div className="mt-auto flex flex-wrap gap-2">
        <Button type="button" disabled>
          <FilePenLineIcon data-icon="inline-start" aria-hidden="true" />
          Entscheidung ändern folgt
        </Button>
        <Button type="button" variant="outline" disabled>
          <ExternalLinkIcon data-icon="inline-start" aria-hidden="true" />
          Anzeige öffnen folgt
        </Button>
      </div>
    </aside>
  );
}

function PreviewDetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="ml-auto truncate text-sm">{value}</span>
    </div>
  );
}

function PreviewSkeleton() {
  return (
    <aside className={previewPanelClassName}>
      <div className="flex items-start gap-3">
        <Skeleton className="size-10" />
        <div className="grid min-w-0 flex-1 gap-2">
          <Skeleton className="h-4 w-4/5" />
          <Skeleton className="h-3 w-2/3" />
        </div>
      </div>
      <div className="flex gap-2">
        <Skeleton className="h-5 w-24 rounded-full" />
        <Skeleton className="h-5 w-20 rounded-full" />
      </div>
      <Separator />
      <div className="grid gap-3">
        <Skeleton className="h-4 w-full" />
        <Skeleton className="h-4 w-5/6" />
        <Skeleton className="h-4 w-3/4" />
        <Skeleton className="h-4 w-4/5" />
      </div>
    </aside>
  );
}
