import { Users, Bookmark, FileText, ScanText, TrendingUp } from "lucide-react"
import { useTranslation } from "react-i18next"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@workspace/ui/components/card"
import { Badge } from "@workspace/ui/components/badge"
import { Skeleton } from "@workspace/ui/components/skeleton"

export type MetricCardsStats = {
  scannedSourcesToday: number
  listedPostings: number
  savedApplications: {
    total: number
    thisWeekDelta: number
  }
  createdApplications: {
    total: number
    thisWeekDelta: number
  }
}

type MetricCardsProps = {
  stats: MetricCardsStats
  loading?: boolean
}

export function MetricCards({ stats, loading = false }: MetricCardsProps) {
  const { t } = useTranslation()
  const {
    scannedSourcesToday,
    listedPostings,
    savedApplications,
    createdApplications,
  } = stats

  return (
    <div className="grid grid-cols-1 gap-4 *:data-[slot=card]:bg-linear-to-t *:data-[slot=card]:from-primary/5 *:data-[slot=card]:to-card *:data-[slot=card]:shadow-xs xl:grid-cols-4 dark:*:data-[slot=card]:bg-card">
      <Card>
        <CardHeader>
          <CardTitle>
            <div className="flex size-7 items-center justify-center rounded-lg border border-success bg-success/20 text-success">
              <ScanText className="size-4" />
            </div>
          </CardTitle>
          <CardDescription>{t("dashboard.todaysScan")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-1">
          <div className="flex flex-wrap items-end gap-2">
            <div className="text-3xl leading-none font-medium tracking-tight tabular-nums">
              {loading ? (
                <Skeleton className="h-8 w-12" />
              ) : (
                scannedSourcesToday
              )}
            </div>
            <div>{t("dashboard.sources")}</div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>
            <div className="flex size-7 items-center justify-center rounded-lg border border-blue-500 bg-blue-500/20 text-blue-500">
              <Users className="size-4" />
            </div>
          </CardTitle>
          <CardDescription>Jobs listed</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-1">
          <div className="flex flex-wrap items-end gap-2">
            <div className="text-3xl leading-none font-medium tracking-tight tabular-nums">
              {loading ? <Skeleton className="h-8 w-12" /> : listedPostings}
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>
            <div className="flex size-7 items-center justify-center rounded-lg border border-warning bg-warning/20 text-warning">
              <Bookmark className="size-4" />
            </div>
          </CardTitle>
          <CardDescription>Applications saved</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-1">
          <div className="relative flex flex-wrap items-end gap-2">
            <div className="text-3xl leading-none font-medium tracking-tight tabular-nums">
              {loading ? (
                <Skeleton className="h-8 w-12" />
              ) : (
                savedApplications.total
              )}
            </div>
            {!loading && savedApplications.thisWeekDelta !== 0 && (
              <Badge className="self-center bg-success">
                <TrendingUp className="size-3" />
                {savedApplications.thisWeekDelta}
              </Badge>
            )}
            {!loading && savedApplications.thisWeekDelta !== 0 && (
              <div className="">this week</div>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>
            <div className="flex size-7 items-center justify-center rounded-lg border border-primary bg-primary/20 text-primary">
              <FileText className="size-4" />
            </div>
          </CardTitle>
          <CardDescription>Applications created</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <div className="text-3xl leading-none font-medium tracking-tight tabular-nums">
              {loading ? (
                <Skeleton className="h-8 w-12" />
              ) : (
                createdApplications.total
              )}
            </div>
            {!loading && createdApplications.thisWeekDelta !== 0 && (
              <Badge className="self-center bg-success">
                <TrendingUp className="size-3" />
                {createdApplications.thisWeekDelta}
              </Badge>
            )}
            {!loading && createdApplications.thisWeekDelta !== 0 && (
              <div className="">this week</div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
