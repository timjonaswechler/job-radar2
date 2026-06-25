import {
  ArchiveIcon,
  FilePenLineIcon,
  InboxIcon,
  ListChecksIcon,
  SendIcon,
  StarIcon,
  type LucideIcon,
} from "lucide-react";

import type { BadgeProps } from "@/components/reui/badge";
import type {
  JobPosting,
  JobPostingApplicationState,
  JobPostingPreparationState,
} from "@/lib/api/job-postings";

export type PostingQueueId =
  | "inbox"
  | "interested"
  | "preparation"
  | "applied"
  | "archive"
  | "all";

export type PostingInboxAnchorId = "new" | "review";

export type PostingQueue = {
  id: PostingQueueId;
  label: string;
  description: string;
  icon: LucideIcon;
};

export type QueueCounts = Record<
  PostingQueueId | "newInbox" | "reviewInbox",
  number
>;

export type StatusBadge = {
  label: string;
  variant: BadgeProps["variant"];
};

export type PostingInboxAnchor = {
  id: PostingInboxAnchorId;
  label: string;
  countKey: "newInbox" | "reviewInbox";
};

export const POSTINGS_BASE_PATH = "/postings";

export const QUEUE_DEFINITIONS = [
  {
    id: "inbox",
    label: "Inbox",
    description: "Anzeigen, die noch eine Entscheidung brauchen.",
    icon: InboxIcon,
  },
  {
    id: "interested",
    label: "Interessant",
    description: "Markierte Anzeigen, für die noch keine Vorbereitung läuft.",
    icon: StarIcon,
  },
  {
    id: "preparation",
    label: "Bewerbung vorbereiten",
    description: "Anzeigen mit aktiver oder fertiger Vorbereitung.",
    icon: FilePenLineIcon,
  },
  {
    id: "applied",
    label: "Beworben / Warten",
    description: "Abgeschickte Bewerbungen und laufende Prozesse.",
    icon: SendIcon,
  },
  {
    id: "archive",
    label: "Archiv",
    description: "Ausgeschlossene oder abgeschlossene Anzeigen.",
    icon: ArchiveIcon,
  },
  {
    id: "all",
    label: "Alle Anzeigen",
    description: "Der komplette Bestand inklusive Archiv.",
    icon: ListChecksIcon,
  },
] satisfies PostingQueue[];

export const INBOX_ANCHORS = [
  { id: "new", label: "Neu", countKey: "newInbox" },
  { id: "review", label: "Zu prüfen", countKey: "reviewInbox" },
] satisfies PostingInboxAnchor[];

export const applicationStateLabels = {
  not_applied: "Nicht beworben",
  submitted: "Eingereicht",
  in_process: "Im Prozess",
  rejected_by_company: "Absage",
  withdrawn_by_me: "Zurückgezogen",
  accepted: "Angenommen",
} satisfies Record<JobPostingApplicationState, string>;

export const preparationStateLabels = {
  not_started: "Nicht gestartet",
  in_progress: "In Vorbereitung",
  ready: "Vorbereitung bereit",
} satisfies Record<JobPostingPreparationState, string>;

export function createQueueCounts(postings: JobPosting[]): QueueCounts {
  const inboxPostings = postings.filter((posting) =>
    isPostingInQueue(posting, "inbox"),
  );

  return {
    inbox: inboxPostings.length,
    interested: postings.filter((posting) =>
      isPostingInQueue(posting, "interested"),
    ).length,
    preparation: postings.filter((posting) =>
      isPostingInQueue(posting, "preparation"),
    ).length,
    applied: postings.filter((posting) => isPostingInQueue(posting, "applied"))
      .length,
    archive: postings.filter((posting) => isPostingInQueue(posting, "archive"))
      .length,
    all: postings.length,
    newInbox: inboxPostings.filter((posting) => posting.readState === "unread")
      .length,
    reviewInbox: inboxPostings.filter((posting) => posting.readState === "read")
      .length,
  };
}

export function isPostingInQueue(
  posting: JobPosting,
  queueId: PostingQueueId,
) {
  if (queueId === "all") return true;

  const archived = isArchivedPosting(posting);

  if (queueId === "archive") return archived;
  if (archived) return false;

  if (queueId === "inbox") {
    return (
      posting.interestState === "undecided" &&
      posting.applicationState === "not_applied"
    );
  }

  if (queueId === "interested") {
    return (
      posting.interestState === "interested" &&
      posting.preparationState === "not_started" &&
      posting.applicationState === "not_applied"
    );
  }

  if (queueId === "preparation") {
    return (
      posting.interestState === "interested" &&
      posting.applicationState === "not_applied" &&
      (posting.preparationState === "in_progress" ||
        posting.preparationState === "ready")
    );
  }

  return (
    posting.applicationState === "submitted" ||
    posting.applicationState === "in_process"
  );
}

export function isArchivedPosting(posting: JobPosting) {
  return (
    posting.interestState === "dismissed" ||
    posting.applicationState === "rejected_by_company" ||
    posting.applicationState === "withdrawn_by_me" ||
    posting.applicationState === "accepted"
  );
}

export function getQueueDefinition(queueId: PostingQueueId) {
  return (
    QUEUE_DEFINITIONS.find((queue) => queue.id === queueId) ??
    QUEUE_DEFINITIONS[0]
  );
}

export function getPostingQueueUrl(queueId: PostingQueueId) {
  if (queueId === "inbox") return `${POSTINGS_BASE_PATH}/inbox`;

  return `${POSTINGS_BASE_PATH}/${queueId}`;
}

export function getPostingInboxAnchorUrl(anchorId: PostingInboxAnchorId) {
  return `${POSTINGS_BASE_PATH}/inbox/${anchorId}`;
}

export function getPostingQueueIdFromPath(pathname: string): PostingQueueId {
  const segment = pathname.split("/").filter(Boolean)[1];

  if (!segment) return "inbox";

  const queue = QUEUE_DEFINITIONS.find((definition) => definition.id === segment);

  return queue?.id ?? "inbox";
}

export function getPostingInboxAnchorFromPath(
  pathname: string,
): PostingInboxAnchorId | null {
  const [feature, queue, anchor] = pathname.split("/").filter(Boolean);

  if (feature !== "postings" || queue !== "inbox") return null;

  return INBOX_ANCHORS.some((item) => item.id === anchor)
    ? (anchor as PostingInboxAnchorId)
    : null;
}

export function isPostingQueuePathActive(
  pathname: string,
  queueId: PostingQueueId,
) {
  return getPostingQueueIdFromPath(pathname) === queueId;
}

export function isPostingInboxAnchorPathActive(
  pathname: string,
  anchorId: PostingInboxAnchorId,
) {
  return getPostingInboxAnchorFromPath(pathname) === anchorId;
}

export function getPrimaryQueueLabel(posting: JobPosting) {
  const queue = QUEUE_DEFINITIONS.find(
    (definition) =>
      definition.id !== "all" && isPostingInQueue(posting, definition.id),
  );

  return queue?.label ?? "Alle Anzeigen";
}

export function getWorkflowBadge(posting: JobPosting): StatusBadge {
  if (isArchivedPosting(posting)) {
    return { label: "Archiv", variant: "outline" };
  }

  if (
    posting.applicationState === "submitted" ||
    posting.applicationState === "in_process"
  ) {
    return { label: "Beworben / Warten", variant: "info-light" };
  }

  if (
    posting.interestState === "interested" &&
    (posting.preparationState === "in_progress" ||
      posting.preparationState === "ready")
  ) {
    return { label: "Bewerbung vorbereiten", variant: "warning-light" };
  }

  if (posting.interestState === "interested") {
    return { label: "Interessant", variant: "success-light" };
  }

  if (posting.readState === "unread") {
    return { label: "Neue Entscheidung", variant: "primary-light" };
  }

  return { label: "Zu prüfen", variant: "secondary" };
}

export function getSourceLabel(posting: JobPosting) {
  const sourceName =
    posting.primarySource?.sourceNameSnapshot ??
    posting.sources[0]?.sourceNameSnapshot ??
    "Quelle offen";
  const sourceCount = posting.sources.length;

  if (sourceCount <= 1) return sourceName;

  return `${sourceName} +${sourceCount - 1}`;
}

export function formatLocations(locations: string[]) {
  if (!locations.length) return "Ort offen";
  if (locations.length <= 2) return locations.join(", ");

  return `${locations[0]} +${locations.length - 1}`;
}

export function formatRelativeDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Zeit offen";

  const diffInSeconds = Math.round((date.getTime() - Date.now()) / 1000);
  const absoluteDiff = Math.abs(diffInSeconds);
  const formatter = new Intl.RelativeTimeFormat("de", { numeric: "auto" });

  if (absoluteDiff < 60) return formatter.format(diffInSeconds, "second");
  if (absoluteDiff < 60 * 60) {
    return formatter.format(Math.round(diffInSeconds / 60), "minute");
  }
  if (absoluteDiff < 60 * 60 * 24) {
    return formatter.format(Math.round(diffInSeconds / (60 * 60)), "hour");
  }
  if (absoluteDiff < 60 * 60 * 24 * 7) {
    return formatter.format(Math.round(diffInSeconds / (60 * 60 * 24)), "day");
  }

  return new Intl.DateTimeFormat("de", {
    day: "2-digit",
    month: "short",
  }).format(date);
}

export function formatAbsoluteDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Zeit offen";

  return new Intl.DateTimeFormat("de", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}
