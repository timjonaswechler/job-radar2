import { invoke } from "@tauri-apps/api/core"
import type { MetricCardsStats } from "@/components/dashboard/metric-cards"

export async function getDashboardStats(): Promise<MetricCardsStats> {
  return invoke<MetricCardsStats>("get_dashboard_stats")
}
