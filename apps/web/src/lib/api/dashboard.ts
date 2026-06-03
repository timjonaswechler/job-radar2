import { invoke } from "@tauri-apps/api/core"
import type { MetricCardsStats } from "@/features/dashboard/components/metric-cards"

export async function getDashboardStats(): Promise<MetricCardsStats> {
  return invoke<MetricCardsStats>("get_dashboard_stats")
}
