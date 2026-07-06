import { useState } from "react";

import { ChevronDownIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Skeleton } from "@/components/ui/skeleton";
import type { PostingDetailLoadState } from "@/features/postings/view-model/posting-item-view-model";
import { cn } from "@/lib/utils";

export function PostingDescription({
  detailState,
  postingId,
  onRetry,
}: {
  detailState: PostingDetailLoadState;
  postingId: number;
  onRetry?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const state =
    detailState.status !== "idle" && detailState.postingId === postingId
      ? detailState
      : ({ status: "idle" } as const);

  if (state.status === "loading") {
    return (
      <section className="rounded-lg border bg-background p-3">
        <DescriptionHeader helper="Ausschreibung wird geladen…" />
        <div className="mt-3 grid gap-2">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-11/12" />
          <Skeleton className="h-4 w-4/5" />
        </div>
      </section>
    );
  }

  if (state.status === "failed") {
    return (
      <section className="rounded-lg border bg-background p-3">
        <DescriptionHeader helper="Laden fehlgeschlagen" />
        <div className="mt-3 flex flex-wrap items-center gap-3 rounded-md border border-dashed bg-muted/20 p-3 text-sm text-muted-foreground">
          <span className="min-w-0 flex-1">{state.message}</span>
          {onRetry ? (
            <Button type="button" variant="outline" size="xs" onClick={onRetry}>
              Erneut laden
            </Button>
          ) : null}
        </div>
      </section>
    );
  }

  if (state.status === "loaded") {
    const descriptionState = state.detail.descriptionState;

    if (descriptionState.status !== "loaded") {
      return (
        <section className="rounded-lg border bg-background p-3">
          <DescriptionHeader helper="Nicht verfügbar" />
          <div className="mt-3 rounded-md border border-dashed bg-muted/20 p-3 text-sm text-muted-foreground">
            {descriptionState.message}
          </div>
        </section>
      );
    }

    return (
      <Collapsible
        open={open}
        onOpenChange={setOpen}
        className="rounded-lg border bg-background p-3"
      >
        <div className="flex flex-wrap items-start justify-between gap-2">
          <DescriptionHeader helper="Aus der Source geladen" />
          <CollapsibleTrigger
            render={
              <Button
                type="button"
                variant="outline"
                size="xs"
                className="group"
              />
            }
          >
            <ChevronDownIcon
              data-icon="inline-start"
              className="transition-transform group-data-[state=open]:rotate-180"
              aria-hidden="true"
            />
            {open ? "Weniger" : "Mehr"}
          </CollapsibleTrigger>
        </div>
        <p
          className={cn(
            "mt-3 whitespace-pre-wrap text-sm leading-6 text-foreground",
            open ? "hidden" : "line-clamp-4",
          )}
        >
          {descriptionState.text}
        </p>
        <CollapsibleContent className="mt-3 whitespace-pre-wrap rounded-md border bg-muted/20 p-3 text-sm leading-6 text-foreground">
          {descriptionState.text}
        </CollapsibleContent>
      </Collapsible>
    );
  }

  return (
    <section className="rounded-lg border bg-background p-3">
      <DescriptionHeader helper="Noch nicht geladen" />
      <p className="mt-1 max-w-4xl text-sm leading-6 text-muted-foreground">
        Klicke die Anzeige in der Liste an, um den Ausschreibungstext zu laden
        und den Neu-Status als gelesen zu markieren.
      </p>
    </section>
  );
}

function DescriptionHeader({ helper }: { helper: string }) {
  return (
    <div className="min-w-0">
      <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        Ausschreibungstext
      </p>
      <p className="mt-1 text-sm text-muted-foreground">{helper}</p>
    </div>
  );
}
