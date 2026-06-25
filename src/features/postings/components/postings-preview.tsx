import { useState, type ReactNode } from "react";

import {
  ArchiveIcon,
  BriefcaseBusinessIcon,
  CalendarDaysIcon,
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  Clock3Icon,
  EllipsisVerticalIcon,
  ExternalLinkIcon,
  FilePenLineIcon,
  FileTextIcon,
  FolderOpenIcon,
  ForwardIcon,
  MailOpenIcon,
  PanelRightIcon,
  PaperclipIcon,
  PinIcon,
  ReplyAllIcon,
  ReplyIcon,
  TagIcon,
  Trash2Icon,
  XIcon,
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
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { PostingPreviewViewModel } from "@/features/postings/postings-view-model";
import { cn } from "@/lib/utils";

type PostingsPreviewProps = {
  loading: boolean;
  posting: PostingPreviewViewModel | null;
};

type SummaryCard = {
  id: string;
  label: string;
  value: string;
  helper: string;
  icon: LucideIcon;
};

type AttachmentPlaceholder = {
  id: string;
  name: string;
  helper: string;
};

const previewPanelClassName =
  "flex h-full min-h-0 min-w-0 flex-col gap-3 overflow-y-auto px-2 py-3";

const attachmentPlaceholders = [
  {
    id: "cv",
    name: "Lebenslauf",
    helper: "Platzhalter",
  },
  {
    id: "certificates",
    name: "Zeugnisse",
    helper: "Noch nicht verknüpft",
  },
  {
    id: "cover-letter",
    name: "Anschreiben",
    helper: "später pro Anzeige",
  },
] satisfies AttachmentPlaceholder[];

const processSteps = [
  { step: 1, label: "Gefunden" },
  { step: 2, label: "Sichtung" },
  { step: 3, label: "Vorbereitung" },
  { step: 4, label: "Beworben" },
];

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
      <PreviewToolbar />
      <Separator />

      <PostingHeader posting={posting} />
      <PreviewSummaryGrid posting={posting} />

      <PreviewProcessStepper posting={posting} />
      <PreviewAttachments />
      <PostingDescriptionPlaceholder />

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

      <div className="flex flex-col gap-3 text-sm">
        <div className="flex items-start gap-2 rounded-md border border-dashed p-3">
          <Clock3Icon
            aria-hidden="true"
            className="mt-0.5 size-4 shrink-0 text-muted-foreground"
          />
          <div className="grid gap-1">
            <div className="font-medium">Detaildaten folgen später</div>
            <p className="text-muted-foreground">
              Ausschreibungstext, echte Unterlagen-Verknüpfungen und ein
              präziser Vorbereitungsschritt brauchen noch Backend-Daten. Dieses
              Panel zeigt dafür schon die vorgesehene Struktur.
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
  const activeStep = getPreviewProcessStep(posting);

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

function PreviewAttachments() {
  const [open, setOpen] = useState(true);

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border bg-background p-3"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Bewerbungsunterlagen
          </p>
          <p className="text-xs text-muted-foreground">
            Platzhalter für schnelle Links zu Lebenslauf, Zeugnissen und
            Anschreiben.
          </p>
        </div>
        <CollapsibleTrigger
          render={
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="group p-0 font-normal text-muted-foreground hover:bg-transparent"
            />
          }
        >
          <PaperclipIcon data-icon="inline-start" aria-hidden="true" />
          {open ? "Ausblenden" : "Anzeigen"}
          <ChevronDownIcon
            data-icon="inline-end"
            className="transition-transform group-data-[state=open]:rotate-180"
            aria-hidden="true"
          />
        </CollapsibleTrigger>
      </div>

      <CollapsibleContent className="mt-3">
        <div className="flex flex-wrap gap-2">
          {attachmentPlaceholders.map((attachment) => (
            <Button
              key={attachment.id}
              type="button"
              size="xs"
              variant="secondary"
              disabled
            >
              <FileTextIcon data-icon="inline-start" aria-hidden="true" />
              <span className="font-normal">{attachment.name}</span>
              <span className="font-normal text-muted-foreground">
                {attachment.helper}
              </span>
            </Button>
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

function PostingDescriptionPlaceholder() {
  const [open, setOpen] = useState(false);

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border bg-background p-3"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Ausschreibungstext
          </p>
          <p className="mt-1 max-w-4xl text-sm leading-6 text-muted-foreground">
            Noch nicht geladen. Später kann diese Fläche den Anfang des
            Ausschreibungstextes zeigen und bei Bedarf aufgeklappt werden.
          </p>
        </div>
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
      <CollapsibleContent className="mt-3 rounded-md border border-dashed bg-muted/20 p-3 text-sm text-muted-foreground">
        Backend fehlt noch: Ausschreibungstext abrufen, speichern, Ladezustand
        abbilden und Fehler menschlich anzeigen. Das sollte später eine
        explizite Nutzeraktion sein, nicht automatisch durch reine Auswahl der
        Zeile passieren.
      </CollapsibleContent>
    </Collapsible>
  );
}

function PreviewToolbar() {
  return (
    <div className="flex items-center gap-3">
      <div className="flex items-center gap-3">
        <DisabledToolbarButton label="Detail schließen folgt">
          <XIcon aria-hidden="true" />
        </DisabledToolbarButton>
        <Separator
          className="h-4 data-vertical:self-center"
          orientation="vertical"
        />
        <div className="flex items-center gap-0">
          <DisabledToolbarButton label="Vorherige Anzeige folgt">
            <ChevronLeftIcon aria-hidden="true" />
          </DisabledToolbarButton>
          <DisabledToolbarButton label="Nächste Anzeige folgt">
            <ChevronRightIcon aria-hidden="true" />
          </DisabledToolbarButton>
        </div>
      </div>

      <div className="ml-auto flex items-center gap-2">
        <DisabledToolbarButton label="Anzeige anpinnen folgt">
          <PinIcon aria-hidden="true" />
        </DisabledToolbarButton>
        <DisabledToolbarButton label="Archivieren folgt">
          <ArchiveIcon aria-hidden="true" />
        </DisabledToolbarButton>
        <DisabledToolbarButton label="Notiz oder Antwort folgt">
          <ReplyIcon aria-hidden="true" />
        </DisabledToolbarButton>
        <MoreActionsMenu />
        <Separator
          className="h-4 data-vertical:self-center"
          orientation="vertical"
        />
        <DisabledToolbarButton label="Entfernen folgt">
          <Trash2Icon aria-hidden="true" className="text-destructive" />
        </DisabledToolbarButton>
      </div>
    </div>
  );
}

function DisabledToolbarButton({
  children,
  label,
}: {
  children: ReactNode;
  label: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <span className="inline-flex size-7">
            <Button type="button" variant="ghost" size="icon" disabled>
              {children}
              <span className="sr-only">{label}</span>
            </Button>
          </span>
        }
      />
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

function MoreActionsMenu() {
  return (
    <Tooltip>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button type="button" variant="ghost" size="icon-sm">
              <EllipsisVerticalIcon aria-hidden="true" />
              <span className="sr-only">Weitere Aktionen</span>
            </Button>
          }
        />
        <DropdownMenuContent align="end" className="w-56">
          <DropdownMenuGroup>
            <DropdownMenuItem disabled>
              <ReplyAllIcon aria-hidden="true" />
              Notiz hinzufügen folgt
            </DropdownMenuItem>
            <DropdownMenuItem disabled>
              <ForwardIcon aria-hidden="true" />
              Teilen folgt
            </DropdownMenuItem>
          </DropdownMenuGroup>
          <DropdownMenuSeparator />
          <DropdownMenuGroup>
            <DropdownMenuItem disabled>
              <MailOpenIcon aria-hidden="true" />
              Als ungelesen markieren folgt
            </DropdownMenuItem>
            <DropdownMenuItem disabled>
              <TagIcon aria-hidden="true" />
              Label hinzufügen folgt
            </DropdownMenuItem>
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <TooltipContent>Weitere Aktionen</TooltipContent>
    </Tooltip>
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
        <Skeleton className="h-20 w-full rounded-lg" />
        <Skeleton className="h-16 w-full rounded-lg" />
        <Skeleton className="h-24 w-full rounded-lg" />
      </div>
    </aside>
  );
}

function createSummaryCards(posting: PostingPreviewViewModel): SummaryCard[] {
  return [
    {
      id: "source",
      label: "Quelle",
      value: getPreviewDetailValue(posting, "Primäre Quelle"),
      helper: "Provenienz",
      icon: ExternalLinkIcon,
    },
    {
      id: "application",
      label: "Bewerbung",
      value: getPreviewDetailValue(posting, "Bewerbungsstand"),
      helper: getPreviewDetailValue(posting, "Vorbereitung"),
      icon: BriefcaseBusinessIcon,
    },
    {
      id: "last-seen",
      label: "Zuletzt gesehen",
      value: getPreviewDetailValue(posting, "Zuletzt gesehen"),
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

function getPreviewProcessStep(posting: PostingPreviewViewModel) {
  const application = getPreviewDetailValue(posting, "Bewerbungsstand");
  const preparation = getPreviewDetailValue(posting, "Vorbereitung");
  const queue = getPreviewDetailValue(posting, "Queue");

  if (application !== "Nicht beworben" && application !== "—") return 4;
  if (preparation !== "Nicht gestartet" && preparation !== "—") return 3;
  if (queue === "Interessant" || queue === "Bewerbung vorbereiten") return 2;

  return 2;
}

function getPreviewDetailValue(
  posting: PostingPreviewViewModel,
  label: string,
) {
  return posting.detailRows.find((row) => row.label === label)?.value ?? "—";
}
