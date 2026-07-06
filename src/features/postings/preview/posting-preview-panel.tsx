import {
  BriefcaseBusinessIcon,
  CalendarDaysIcon,
  Clock3Icon,
  ExternalLinkIcon,
  FilePenLineIcon,
  FolderOpenIcon,
  type LucideIcon,
} from "lucide-react";

import { Badge } from "@/components/reui/badge";
import {
  Stepper,
  StepperIndicator,
  StepperItem,
  StepperNav,
  StepperSeparator,
} from "@/components/reui/stepper";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { PreviewAttachments } from "@/features/postings/preview/posting-preview-attachments";
import { PostingDescription } from "@/features/postings/preview/posting-preview-description";
import { previewPanelClassName } from "@/features/postings/preview/posting-preview-layout";
import {
  PostingPreviewEmptyState,
  PreviewSkeleton,
} from "@/features/postings/preview/posting-preview-states";
import { PreviewToolbar } from "@/features/postings/preview/posting-preview-toolbar";
import type {
  PostingDetailLoadState,
  PostingPreviewViewModel,
} from "@/features/postings/view-model/posting-item-view-model";
import { cn } from "@/lib/utils";

type PostingPreviewPanelProps = {
  detailState: PostingDetailLoadState;
  loading: boolean;
  posting: PostingPreviewViewModel | null;
  onRetryDetail?: () => void;
};

type SummaryCard = {
  id: string;
  label: string;
  value: string;
  helper: string;
  icon: LucideIcon;
};

const processSteps = [
  { step: 1, label: "Gefunden" },
  { step: 2, label: "Sichtung" },
  { step: 3, label: "Vorbereitung" },
  { step: 4, label: "Beworben" },
];

export function PostingPreviewPanel({
  detailState,
  loading,
  posting,
  onRetryDetail,
}: PostingPreviewPanelProps) {
  if (loading) return <PreviewSkeleton />;

  if (!posting) return <PostingPreviewEmptyState />;

  return (
    <aside className={previewPanelClassName}>
      <PreviewToolbar />
      <Separator />

      <PostingHeader posting={posting} />
      <PreviewSummaryGrid posting={posting} />

      <PreviewProcessStepper posting={posting} />
      <PreviewAttachments />
      <PostingDescription
        detailState={detailState}
        postingId={posting.id}
        onRetry={onRetryDetail}
      />

      <Separator />

      <PreviewMetadataRows posting={posting} />
      <PreviewFutureDetailsNotice />
      <PreviewFooterActions />
    </aside>
  );
}

function PostingHeader({ posting }: { posting: PostingPreviewViewModel }) {
  return (
    <div className="flex items-start gap-3">
      <div className="flex size-10 shrink-0 items-center justify-center rounded-md bg-muted text-sm font-medium">
        {posting.companyInitials}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <div className="min-w-0 truncate font-medium leading-5">
            {posting.title}
          </div>
          <div className="flex min-w-fit flex-wrap gap-1.5">
            {posting.badges.map((badge) => (
              <Badge key={badge.label} variant={badge.variant} radius="full">
                {badge.label}
              </Badge>
            ))}
          </div>
        </div>
        <div className="mt-1 truncate text-xs text-muted-foreground">
          {posting.subtitle}
        </div>
      </div>
    </div>
  );
}

function PreviewSummaryGrid({ posting }: { posting: PostingPreviewViewModel }) {
  const cards = createSummaryCards(posting);

  return (
    <section className="grid grid-cols-2 overflow-hidden border-y border-border bg-muted/20 xl:grid-cols-4 xl:border-x-0">
      {cards.map((card, index) => {
        const Icon = card.icon;
        const hasBottomBorder = index < cards.length - 2;
        const hasLeftBorder = index % 2 === 1;

        return (
          <div
            key={card.id}
            className={cn(
              "min-w-0 border-border/70 p-3.5 sm:p-4 xl:border-b-0 xl:border-l xl:first:border-l-0",
              hasBottomBorder && "border-b",
              hasLeftBorder && "border-l",
            )}
          >
            <div className="flex items-center gap-2">
              <Icon
                aria-hidden="true"
                className="size-3.5 shrink-0 text-muted-foreground"
              />
              <p className="min-w-0 truncate text-xs font-medium text-muted-foreground">
                {card.label}
              </p>
            </div>
            <div className="mt-2 flex min-w-0 flex-col gap-1 xl:flex-row xl:items-end xl:justify-between xl:gap-3">
              <p className="min-w-0 truncate text-lg font-semibold tracking-tight">
                {card.value}
              </p>
              <p className="min-w-0 truncate text-xs text-muted-foreground xl:shrink-0">
                {card.helper}
              </p>
            </div>
          </div>
        );
      })}
    </section>
  );
}

function PreviewProcessStepper({
  posting,
}: {
  posting: PostingPreviewViewModel;
}) {
  const activeStep = posting.workflow.processStep;

  return (
    <section className="rounded-lg border bg-background p-3">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <div>
          <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Prozess-Skizze
          </p>
          <p className="text-xs text-muted-foreground">
            Vorläufig aus bestehenden Posting-Zuständen abgeleitet.
          </p>
        </div>
        <Badge variant="secondary" radius="full">
          Platzhalter
        </Badge>
      </div>

      <Stepper value={activeStep} className="w-full">
        <StepperNav>
          {processSteps.map((item, index) => (
            <StepperItem key={item.step} step={item.step}>
              <Tooltip>
                <TooltipTrigger
                  render={
                    <div className="inline-flex items-center rounded-full" />
                  }
                >
                  <StepperIndicator className="rounded-full">
                    {item.step}
                    <span className="sr-only">{item.label}</span>
                  </StepperIndicator>
                </TooltipTrigger>
                <TooltipContent>{item.label}</TooltipContent>
              </Tooltip>
              {index < processSteps.length - 1 ? <StepperSeparator /> : null}
            </StepperItem>
          ))}
        </StepperNav>
      </Stepper>
    </section>
  );
}

function PreviewMetadataRows({ posting }: { posting: PostingPreviewViewModel }) {
  return (
    <div className="flex flex-col gap-3 text-sm">
      {posting.detailRows.map((row) => (
        <PreviewDetailRow key={row.label} label={row.label} value={row.value} />
      ))}
    </div>
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

function PreviewFutureDetailsNotice() {
  return (
    <div className="flex flex-col gap-3 text-sm">
      <div className="flex items-start gap-2 rounded-md border border-dashed p-3">
        <Clock3Icon
          aria-hidden="true"
          className="mt-0.5 size-4 shrink-0 text-muted-foreground"
        />
        <div className="grid gap-1">
          <div className="font-medium">Detaildaten folgen später</div>
          <p className="text-muted-foreground">
            Ausschreibungstext, echte Unterlagen-Verknüpfungen und ein präziser
            Vorbereitungsschritt brauchen noch Backend-Daten. Dieses Panel zeigt
            dafür schon die vorgesehene Struktur.
          </p>
        </div>
      </div>
    </div>
  );
}

function PreviewFooterActions() {
  return (
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
  );
}

function createSummaryCards(posting: PostingPreviewViewModel): SummaryCard[] {
  const { workflow } = posting;

  return [
    {
      id: "source",
      label: "Quelle",
      value: workflow.primarySourceLabel,
      helper: "Provenienz",
      icon: ExternalLinkIcon,
    },
    {
      id: "application",
      label: "Bewerbung",
      value: workflow.applicationLabel,
      helper: workflow.preparationLabel,
      icon: BriefcaseBusinessIcon,
    },
    {
      id: "last-seen",
      label: "Zuletzt gesehen",
      value: workflow.lastSeenLabel,
      helper: "letzte Sichtung",
      icon: CalendarDaysIcon,
    },
    {
      id: "documents",
      label: "Unterlagen",
      value: "0 verknüpft",
      helper: "Platzhalter",
      icon: FolderOpenIcon,
    },
  ];
}
