import { useEffect, useState } from "react"
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert"
import { MetricCards, type MetricCardsStats } from "./components/metric-cards"
import { getDashboardStats } from "@/lib/api/dashboard"
import { RecentOrders } from "./components/jobs-table"
import { AnalyticsKpiStrip } from "./components/strip"


const emptyStats: MetricCardsStats = {
  scannedSourcesToday: 0,
  listedPostings: 0,
  savedApplications: {
    total: 0,
    thisWeekDelta: 0,
  },
  createdApplications: {
    total: 0,
    thisWeekDelta: 0,
  },
}

export function DashboardPage() {
  const [stats, setStats] = useState<MetricCardsStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false

    getDashboardStats()
      .then((stats) => {
        if (cancelled) return
        setStats(stats)
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setError(String(error))
      })
      .finally(() => {
        if (cancelled) return
        setLoading(false)
      })

    return () => {
      cancelled = true
    }
  }, [])

  return (
    <div className="flex flex-col gap-4">
      {error && (
        <Alert variant="destructive">
          <AlertTitle>Dashboard konnte nicht geladen werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <AnalyticsKpiStrip />
      <MetricCards stats={stats ?? emptyStats} loading={loading} />
      <RecentOrders />
    </div>
  )
}
