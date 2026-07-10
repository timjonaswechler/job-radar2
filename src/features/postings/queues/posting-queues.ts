import {
  ArchiveIcon,
  FilePenLineIcon,
  InboxIcon,
  ListChecksIcon,
  SendIcon,
  StarIcon,
  type LucideIcon,
} from "lucide-react";

import type {
  JobPosting,
  JobPostingQueueCounts,
  JobPostingQueueId,
} from "@/lib/api/job-postings";

export type PostingQueueId = JobPostingQueueId;

export type PostingQueue = {
  id: PostingQueueId;
  label: string;
  description: string;
  icon: LucideIcon;
};

export type QueueCounts = JobPostingQueueCounts;

export const POSTINGS_BASE_PATH = "/postings";

export const EMPTY_QUEUE_COUNTS = {
  inbox: 0,
  interested: 0,
  preparation: 0,
  applied: 0,
  archive: 0,
  all: 0,
  newInbox: 0,
  reviewInbox: 0,
} satisfies QueueCounts;

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

export function isPostingInQueue(posting: JobPosting, queueId: PostingQueueId) {
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

export function getPostingQueueIdFromPath(pathname: string): PostingQueueId {
  const segment = pathname.split("/").filter(Boolean)[1];

  if (!segment) return "inbox";

  const queue = QUEUE_DEFINITIONS.find(
    (definition) => definition.id === segment,
  );

  return queue?.id ?? "inbox";
}

export function isPostingQueuePathActive(
  pathname: string,
  queueId: PostingQueueId,
) {
  const isPostingsPath =
    pathname === POSTINGS_BASE_PATH ||
    pathname.startsWith(`${POSTINGS_BASE_PATH}/`);

  return isPostingsPath && getPostingQueueIdFromPath(pathname) === queueId;
}

export function getPrimaryQueueLabel(posting: JobPosting) {
  const queue = QUEUE_DEFINITIONS.find(
    (definition) =>
      definition.id !== "all" && isPostingInQueue(posting, definition.id),
  );

  return queue?.label ?? "Alle Anzeigen";
}
