import assert from "node:assert/strict";

import { loadPostingDetailForWorkspace } from "@/features/postings/posting-detail-workspace";
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
