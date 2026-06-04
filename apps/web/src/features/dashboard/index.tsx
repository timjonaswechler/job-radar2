import { useEffect, useState } from "react"
import { toast } from "sonner"
import { useLocale } from "@/context/locale-provider-context"
import {
  createDashboardStatsRange,
  getDashboardStats,
} from "@/lib/api/dashboard"
import type { DashboardStats } from "./types"
import { RecentPostingsTable } from "./components/jobs-table"
import { MetricStrip } from "./components/strip"

const emptyStats: DashboardStats = {
  newPostingsThisWeek: {
    count: 0,
    previousWeekCount: 0,
    delta: 0,
  },
  interestingPostings: {
    total: 0,
    newThisWeek: 0,
  },
  applicationsSentThisWeek: {
    count: 0,
    previousWeekCount: 0,
    delta: 0,
  },
  dueFollowUps: {
    total: 0,
  },
}

export function DashboardPage() {
  const { weekStartsOn } = useLocale()
  const [stats, setStats] = useState<DashboardStats | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    const range = createDashboardStatsRange(weekStartsOn)

    getDashboardStats(range)
      .then((stats) => {
        if (cancelled) return
        toast.dismiss("dashboard-load-error")
        setStats(stats)
      })
      .catch((error: unknown) => {
        if (cancelled) return
        toast.error("Dashboard konnte nicht geladen werden", {
          id: "dashboard-load-error",
          description: String(error),
        })
      })
      .finally(() => {
        if (cancelled) return
        setLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [weekStartsOn])

  return (
    <div className="flex flex-col gap-4">
      <MetricStrip stats={stats ?? emptyStats} loading={loading} />
      <RecentPostingsTable />
    </div>
  )
}
