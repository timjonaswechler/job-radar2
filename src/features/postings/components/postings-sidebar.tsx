import { navigateTo } from "@/app/navigation/path";
import { Skeleton } from "@/components/ui/skeleton";
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from "@/components/ui/sidebar";
import {
  createQueueCounts,
  getPostingInboxAnchorUrl,
  getPostingQueueUrl,
  INBOX_ANCHORS,
  isPostingInboxAnchorPathActive,
  isPostingQueuePathActive,
  QUEUE_DEFINITIONS,
  type PostingQueueId,
  type QueueCounts,
} from "@/features/postings/postings-view-model";
import { useJobPostings } from "@/features/postings/use-job-postings";

const primaryQueueIds = [
  "inbox",
  "interested",
  "preparation",
  "applied",
] satisfies PostingQueueId[];

const viewQueueIds = ["archive", "all"] satisfies PostingQueueId[];

export function PostingsSidebar() {
  const { postings, loading, error } = useJobPostings();
  const counts = createQueueCounts(postings);
  const pathname = window.location.pathname;

  return (
    <SidebarGroup>
      <SidebarGroupLabel>Stellenanzeigen</SidebarGroupLabel>
      <SidebarGroupContent className="flex flex-col gap-2">
        <QueueMenu
          counts={counts}
          ids={primaryQueueIds}
          loading={loading}
          pathname={pathname}
          showUnavailableCounts={Boolean(error)}
        />
        <QueueMenu
          counts={counts}
          ids={viewQueueIds}
          loading={loading}
          pathname={pathname}
          showUnavailableCounts={Boolean(error)}
        />
      </SidebarGroupContent>
    </SidebarGroup>
  );
}

function QueueMenu({
  counts,
  ids,
  loading,
  pathname,
  showUnavailableCounts,
}: {
  counts: QueueCounts;
  ids: readonly PostingQueueId[];
  loading: boolean;
  pathname: string;
  showUnavailableCounts: boolean;
}) {
  const queues = QUEUE_DEFINITIONS.filter((queue) => ids.includes(queue.id));

  return (
    <SidebarMenu>
      {queues.map((queue) => {
        const Icon = queue.icon;
        const isActive = isPostingQueuePathActive(pathname, queue.id);

        return (
          <SidebarMenuItem key={queue.id}>
            <SidebarMenuButton
              type="button"
              tooltip={queue.label}
              isActive={isActive}
              onClick={() => navigateTo(getPostingQueueUrl(queue.id))}
            >
              <Icon aria-hidden="true" />
              <span className="min-w-0 flex-1 truncate">{queue.label}</span>
              <PostingQueueCount
                loading={loading}
                unavailable={showUnavailableCounts}
                value={counts[queue.id]}
              />
            </SidebarMenuButton>

            {queue.id === "inbox" ? (
              <SidebarMenuSub className="mr-0 pr-0 gap-px">
                {INBOX_ANCHORS.map((anchor) => (
                  <SidebarMenuSubItem key={anchor.id}>
                    <SidebarMenuSubButton
                      href={getPostingInboxAnchorUrl(anchor.id)}
                      className="h-8 px-2"
                      isActive={isPostingInboxAnchorPathActive(
                        pathname,
                        anchor.id,
                      )}
                      onClick={(event) => {
                        event.preventDefault();
                        navigateTo(getPostingInboxAnchorUrl(anchor.id));
                      }}
                    >
                      <span className="min-w-0 flex-1 truncate">
                        {anchor.label}
                      </span>
                      <PostingQueueCount
                        loading={loading}
                        unavailable={showUnavailableCounts}
                        value={counts[anchor.countKey]}
                      />
                    </SidebarMenuSubButton>
                  </SidebarMenuSubItem>
                ))}
              </SidebarMenuSub>
            ) : null}
          </SidebarMenuItem>
        );
      })}
    </SidebarMenu>
  );
}

function PostingQueueCount({
  loading,
  unavailable,
  value,
}: {
  loading: boolean;
  unavailable: boolean;
  value: number;
}) {
  return (
    <div className="ml-auto flex min-w-5 shrink-0 justify-end text-xs tabular-nums text-muted-foreground">
      {loading ? (
        <Skeleton className="h-3 w-4" />
      ) : (
        formatCount({ loading, unavailable, value })
      )}
    </div>
  );
}

function formatCount({
  loading,
  unavailable,
  value,
}: {
  loading: boolean;
  unavailable: boolean;
  value: number;
}) {
  if (loading) return "…";
  if (unavailable) return "–";

  return value;
}
