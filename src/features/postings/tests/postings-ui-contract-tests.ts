import assert from "node:assert/strict";

import {
  createQueueCounts,
  getPrimaryQueueLabel,
  isPostingInQueue,
  isPostingQueuePathActive,
  type PostingQueueId,
} from "@/features/postings/queues/posting-queues";
import {
  createPostingItemViewModel,
  getPreviewWorkflowProcessStep,
} from "@/features/postings/view-model/posting-item-view-model";
import { loadPostingDetailForWorkspace } from "@/features/postings/workspace/load-posting-detail";
import type {
  JobPosting,
  JobPostingDetail,
} from "@/lib/api/job-postings";

const unreadPosting = createPosting({
  id: 7,
  readState: "unread",
  descriptionText: null,
});
const untouchedPosting = createPosting({ id: 8, readState: "unread" });
const loadedDetail = createPostingDetail({
  ...unreadPosting,
  readState: "read",
  descriptionText: "Loaded job description",
});

let requestedPostingId: number | null = null;
let refreshCountCalls = 0;
const appliedPostingLists: JobPosting[][] = [];

const result = await loadPostingDetailForWorkspace({
  activeQueueId: "inbox",
  currentPostings: [unreadPosting, untouchedPosting],
  postingId: unreadPosting.id,
  getPostingDetail: async (postingId) => {
    requestedPostingId = postingId;
    return loadedDetail;
  },
  setPostings: (updater) => {
    appliedPostingLists.push(updater([unreadPosting, untouchedPosting]));
  },
  refreshCounts: async () => {
    refreshCountCalls += 1;
  },
});

assert.equal(
  requestedPostingId,
  unreadPosting.id,
  "selecting a posting must call the detail endpoint for that posting",
);
assert.equal(result.descriptionState.status, "loaded");
assert.equal(result.descriptionState.text, "Loaded job description");
assert.equal(appliedPostingLists.length, 1);
assert.equal(
  appliedPostingLists[0]?.[0]?.readState,
  "read",
  "the loaded detail returned by the backend must replace the unread list row",
);
assert.equal(
  appliedPostingLists[0]?.[0]?.descriptionText,
  "Loaded job description",
  "the loaded description must be available to the selected preview/list state",
);
assert.equal(
  appliedPostingLists[0]?.[1]?.id,
  untouchedPosting.id,
  "loading one posting must not rewrite unrelated rows",
);
assert.equal(
  refreshCountCalls,
  1,
  "marking an inbox posting as read via detail loading must refresh queue counts",
);

assert.equal(isPostingQueuePathActive("/postings", "inbox"), true);
assert.equal(isPostingQueuePathActive("/postings/inbox", "inbox"), true);
assert.equal(isPostingQueuePathActive("/settings", "inbox"), false);
assert.equal(isPostingQueuePathActive("/postings-extra", "inbox"), false);

const queuePostings = {
  unreadInbox: createPosting({ id: 101, readState: "unread" }),
  readInbox: createPosting({ id: 102, readState: "read" }),
  interested: createPosting({ id: 103, interestState: "interested" }),
  preparationInProgress: createPosting({
    id: 104,
    interestState: "interested",
    preparationState: "in_progress",
  }),
  preparationReady: createPosting({
    id: 105,
    interestState: "interested",
    preparationState: "ready",
  }),
  appliedSubmitted: createPosting({ id: 106, applicationState: "submitted" }),
  appliedInProcess: createPosting({ id: 107, applicationState: "in_process" }),
  archiveDismissed: createPosting({ id: 108, interestState: "dismissed" }),
  archiveRejected: createPosting({
    id: 109,
    applicationState: "rejected_by_company",
  }),
  archiveAcceptedWithPreparation: createPosting({
    id: 110,
    interestState: "interested",
    preparationState: "ready",
    applicationState: "accepted",
  }),
};

assertQueues(queuePostings.unreadInbox, ["inbox", "all"]);
assertQueues(queuePostings.readInbox, ["inbox", "all"]);
assertQueues(queuePostings.interested, ["interested", "all"]);
assertQueues(queuePostings.preparationInProgress, ["preparation", "all"]);
assertQueues(queuePostings.preparationReady, ["preparation", "all"]);
assertQueues(queuePostings.appliedSubmitted, ["applied", "all"]);
assertQueues(queuePostings.appliedInProcess, ["applied", "all"]);
assertQueues(queuePostings.archiveDismissed, ["archive", "all"]);
assertQueues(queuePostings.archiveRejected, ["archive", "all"]);
assertQueues(
  queuePostings.archiveAcceptedWithPreparation,
  ["archive", "all"],
  "archive state must take precedence over preparation/applied queues",
);

assert.equal(getPrimaryQueueLabel(queuePostings.unreadInbox), "Inbox");
assert.equal(getPrimaryQueueLabel(queuePostings.interested), "Interessant");
assert.equal(
  getPrimaryQueueLabel(queuePostings.preparationReady),
  "Bewerbung vorbereiten",
);
assert.equal(
  getPrimaryQueueLabel(queuePostings.appliedInProcess),
  "Beworben / Warten",
);
assert.equal(getPrimaryQueueLabel(queuePostings.archiveDismissed), "Archiv");

assert.deepEqual(createQueueCounts(Object.values(queuePostings)), {
  inbox: 2,
  interested: 1,
  preparation: 2,
  applied: 2,
  archive: 3,
  all: 10,
  newInbox: 1,
  reviewInbox: 1,
});

const preparationViewModel = createPostingItemViewModel(
  queuePostings.preparationReady,
);
const { lastSeenLabel, ...preparationWorkflow } =
  preparationViewModel.preview.workflow;
assert.match(lastSeenLabel, /2026/);
assert.deepEqual(preparationWorkflow, {
  queueLabel: "Bewerbung vorbereiten",
  applicationLabel: "Nicht beworben",
  preparationLabel: "Vorbereitung bereit",
  primarySourceLabel: "Acme Careers",
  processStep: 3,
});
assert.deepEqual(preparationViewModel.preview.detailRows.slice(0, 4), [
  { label: "Queue", value: preparationViewModel.preview.workflow.queueLabel },
  {
    label: "Bewerbungsstand",
    value: preparationViewModel.preview.workflow.applicationLabel,
  },
  {
    label: "Vorbereitung",
    value: preparationViewModel.preview.workflow.preparationLabel,
  },
  {
    label: "Primäre Quelle",
    value: preparationViewModel.preview.workflow.primarySourceLabel,
  },
]);

assert.equal(
  createPostingItemViewModel(queuePostings.interested).preview.workflow
    .processStep,
  2,
);
assert.equal(
  getPreviewWorkflowProcessStep({
    queueLabel: "Custom Queue Copy",
    applicationLabel: "Eingereicht",
    preparationLabel: "Nicht gestartet",
  }),
  4,
);
assert.equal(
  createPostingItemViewModel(queuePostings.appliedSubmitted).preview.workflow
    .processStep,
  4,
);
assert.equal(
  createPostingItemViewModel(queuePostings.archiveAcceptedWithPreparation).preview
    .workflow.queueLabel,
  "Archiv",
);

function assertQueues(
  posting: JobPosting,
  expectedQueueIds: PostingQueueId[],
  message = `posting ${posting.id} queue membership`,
) {
  const actualQueueIds = ([
    "inbox",
    "interested",
    "preparation",
    "applied",
    "archive",
    "all",
  ] satisfies PostingQueueId[]).filter((queueId) =>
    isPostingInQueue(posting, queueId),
  );
  assert.deepEqual(actualQueueIds, expectedQueueIds, message);
}

function createPosting(overrides: Partial<JobPosting> = {}): JobPosting {
  return {
    id: 1,
    title: "Product Engineer",
    company: "Acme GmbH",
    locations: ["Berlin"],
    descriptionText: null,
    readState: "unread",
    interestState: "undecided",
    preparationState: "not_started",
    applicationState: "not_applied",
    firstSeenAt: "2026-07-05T10:00:00.000Z",
    lastSeenAt: "2026-07-05T11:00:00.000Z",
    createdAt: "2026-07-05T10:00:00.000Z",
    updatedAt: "2026-07-05T10:00:00.000Z",
    primarySource: {
      id: 1,
      sourceKey: "acme",
      sourceNameSnapshot: "Acme Careers",
      url: "https://example.test/jobs/1",
      firstSeenAt: "2026-07-05T10:00:00.000Z",
      lastSeenAt: "2026-07-05T11:00:00.000Z",
    },
    sources: [],
    ...overrides,
  };
}

function createPostingDetail(posting: JobPosting): JobPostingDetail {
  return {
    ...posting,
    descriptionState: {
      status: "loaded",
      text: posting.descriptionText ?? "Loaded job description",
      diagnostics: [],
    },
  };
}
