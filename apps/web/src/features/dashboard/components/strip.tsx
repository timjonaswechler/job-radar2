import type { ReactNode } from "react"
import type { LucideIcon } from "lucide-react"
import {
  ArrowDownRight,
  ArrowRight,
  ArrowUpRight,
  BellRing,
  Bookmark,
  BriefcaseBusiness,
  Send,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardAction,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { useLocale } from "@/context/locale-provider-context"
import { cn } from "@/lib/utils"
import type { DashboardStats } from "../types"

type MetricStripProps = {
  stats: DashboardStats
  loading?: boolean
}

type KpiCardProps = {
  title: string
  value: string
  icon: LucideIcon
  badge: ReactNode
  detail: ReactNode
  loading: boolean
}

function TrendBadge({
  delta,
  formatter,
}: {
  delta: number
  formatter: Intl.NumberFormat
}) {
  const Icon =
    delta > 0 ? ArrowUpRight : delta < 0 ? ArrowDownRight : ArrowRight
  const variant =
    delta < 0 ? "destructive" : delta === 0 ? "secondary" : "default"

  return (
    <Badge
      variant={variant}
      className={cn(
        delta > 0 &&
          "bg-green-500/10 text-green-700 dark:bg-green-500/15 dark:text-green-300"
      )}
    >
      <Icon />
      {delta > 0
        ? `+${formatter.format(delta)}`
        : delta === 0
          ? "±0"
          : formatter.format(delta)}
    </Badge>
  )
}

function KpiCard({
  title,
  value,
  icon: Icon,
  badge,
  detail,
  loading,
}: KpiCardProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="font-normal text-sm">{title}</CardTitle>
        <CardAction className="text-muted-foreground">
          <Icon className="size-4" />
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex items-center justify-between gap-4">
          <div className="text-2xl leading-none tracking-tight tabular-nums">
            {loading ? <Skeleton className="h-7 w-14" /> : value}
          </div>
          {!loading && badge}
        </div>

        <div className="flex items-center gap-2 text-muted-foreground text-xs">
          {loading ? <Skeleton className="h-4 w-28" /> : detail}
        </div>
      </CardContent>
    </Card>
  )
}

export function MetricStrip({ stats, loading = false }: MetricStripProps) {
  const { t } = useTranslation()
  const { intlLocale } = useLocale()
  const numberFormatter = new Intl.NumberFormat(intlLocale)

  return (
    <div className="overflow-hidden rounded-xl bg-card shadow-xs ring-1 ring-foreground/10">
      <div className="grid gap-px bg-border *:data-[slot=card]:rounded-none *:data-[slot=card]:ring-0 md:grid-cols-2 xl:grid-cols-4">
        <KpiCard
          title={t("dashboard.kpis.newPostings.title")}
          value={numberFormatter.format(stats.newPostingsThisWeek.count)}
          icon={BriefcaseBusiness}
          badge={
            <TrendBadge
              delta={stats.newPostingsThisWeek.delta}
              formatter={numberFormatter}
            />
          }
          detail={t("dashboard.kpis.comparedToPreviousWeek")}
          loading={loading}
        />

        <KpiCard
          title={t("dashboard.kpis.interestingPostings.title")}
          value={numberFormatter.format(stats.interestingPostings.total)}
          icon={Bookmark}
          badge={
            stats.interestingPostings.newThisWeek > 0 ? (
              <Badge className="bg-green-500/10 text-green-700 dark:bg-green-500/15 dark:text-green-300">
                <ArrowUpRight />
                +{numberFormatter.format(stats.interestingPostings.newThisWeek)}
              </Badge>
            ) : (
              <Badge variant="secondary">±0</Badge>
            )
          }
          detail={t("dashboard.kpis.interestingPostings.newThisWeek", {
            count: stats.interestingPostings.newThisWeek,
          })}
          loading={loading}
        />

        <KpiCard
          title={t("dashboard.kpis.applicationsSent.title")}
          value={numberFormatter.format(stats.applicationsSentThisWeek.count)}
          icon={Send}
          badge={
            <TrendBadge
              delta={stats.applicationsSentThisWeek.delta}
              formatter={numberFormatter}
            />
          }
          detail={t("dashboard.kpis.comparedToPreviousWeek")}
          loading={loading}
        />

        <KpiCard
          title={t("dashboard.kpis.followUpsDue.title")}
          value={numberFormatter.format(stats.dueFollowUps.total)}
          icon={BellRing}
          badge={
            stats.dueFollowUps.total > 0 ? (
              <Badge variant="destructive">{t("dashboard.kpis.open")}</Badge>
            ) : (
              <Badge className="bg-green-500/10 text-green-700 dark:bg-green-500/15 dark:text-green-300">
                {t("dashboard.kpis.clear")}
              </Badge>
            )
          }
          detail={t("dashboard.kpis.followUpsDue.dueUntilToday")}
          loading={loading}
        />
      </div>
    </div>
  )
}
