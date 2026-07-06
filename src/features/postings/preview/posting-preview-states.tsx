import { PanelRightIcon } from "lucide-react";

import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";

import { previewPanelClassName } from "./posting-preview-layout";

export function PostingPreviewEmptyState() {
  return (
    <aside className={previewPanelClassName}>
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <PanelRightIcon aria-hidden="true" />
          </EmptyMedia>
          <EmptyTitle>Keine Anzeige ausgewählt</EmptyTitle>
          <EmptyDescription>
            Wähle links eine Queue und in der Mitte eine Anzeige aus. Bei leeren
            Queues bleibt das Detailpanel als ruhiger Platzhalter sichtbar.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    </aside>
  );
}

export function PreviewSkeleton() {
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
        <Skeleton className="h-20 w-full rounded-lg" />
        <Skeleton className="h-16 w-full rounded-lg" />
        <Skeleton className="h-24 w-full rounded-lg" />
      </div>
    </aside>
  );
}
