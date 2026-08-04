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

export function isArchivedPosting(posting: JobPosting) {
  return posting.primaryQueue === "archive";
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
  return getQueueDefinition(posting.primaryQueue).label;
}
