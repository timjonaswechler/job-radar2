import { invoke } from "@tauri-apps/api/core"
import {
  addWeeks,
  endOfDay,
  format,
  startOfWeek,
  subWeeks,
} from "date-fns"

import type {
  DashboardPosting,
  DashboardStats,
} from "@/features/dashboard/types"
import type { WeekStartsOn } from "@/lib/i18n/language"

export type DashboardStatsRange = {
  currentWeekStart: string
  currentWeekEnd: string
  previousWeekStart: string
  previousWeekEnd: string
  currentWeekStartDate: string
  currentWeekEndDate: string
  previousWeekStartDate: string
  previousWeekEndDate: string
  dueUntil: string
}

export function createDashboardStatsRange(
  weekStartsOn: WeekStartsOn,
  now = new Date()
): DashboardStatsRange {
  const currentWeekStart = startOfWeek(now, { weekStartsOn })
  const currentWeekEnd = addWeeks(currentWeekStart, 1)
  const previousWeekStart = subWeeks(currentWeekStart, 1)

  return {
    currentWeekStart: currentWeekStart.toISOString(),
    currentWeekEnd: currentWeekEnd.toISOString(),
    previousWeekStart: previousWeekStart.toISOString(),
    previousWeekEnd: currentWeekStart.toISOString(),
    currentWeekStartDate: format(currentWeekStart, "yyyy-MM-dd"),
    currentWeekEndDate: format(currentWeekEnd, "yyyy-MM-dd"),
    previousWeekStartDate: format(previousWeekStart, "yyyy-MM-dd"),
    previousWeekEndDate: format(currentWeekStart, "yyyy-MM-dd"),
    dueUntil: endOfDay(now).toISOString(),
  }
}

export async function getDashboardStats(
  range: DashboardStatsRange
): Promise<DashboardStats> {
  return invoke<DashboardStats>("get_dashboard_stats", { range })
}

export async function getDashboardPostings(): Promise<DashboardPosting[]> {
  return invoke<DashboardPosting[]>("get_dashboard_postings")
}
