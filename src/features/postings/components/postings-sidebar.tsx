import { navigateTo } from "@/app/navigation/path";
import { Skeleton } from "@/components/ui/skeleton";
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import {
  getPostingQueueUrl,
  isPostingQueuePathActive,
  QUEUE_DEFINITIONS,
  type PostingQueueId,
  type QueueCounts,
} from "@/features/postings/postings-view-model";
import { usePostingsWorkspace } from "@/features/postings/postings-workspace-provider";

const primaryQueueIds = [
  "inbox",
  "interested",
  "preparation",
  "applied",
] satisfies PostingQueueId[];

const viewQueueIds = ["archive", "all"] satisfies PostingQueueId[];

export function PostingsSidebar() {
  const { counts, countsLoading, countsError } = usePostingsWorkspace();
  const pathname = window.location.pathname;

  return (
    <SidebarGroup>
      <SidebarGroupLabel>Stellenanzeigen</SidebarGroupLabel>
      <SidebarGroupContent className="flex flex-col gap-2">
        <QueueMenu
          counts={counts}
          ids={primaryQueueIds}
          loading={countsLoading}
          pathname={pathname}
          showUnavailableCounts={Boolean(countsError)}
        />
        <QueueMenu
          counts={counts}
          ids={viewQueueIds}
          loading={countsLoading}
          pathname={pathname}
          showUnavailableCounts={Boolean(countsError)}
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
