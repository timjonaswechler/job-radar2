export type DashboardPeriodMetric = {
  count: number
  previousWeekCount: number
  delta: number
}

export type DashboardInterestingPostingsMetric = {
  total: number
  newThisWeek: number
}

export type DashboardDueFollowUpsMetric = {
  total: number
}

export type DashboardStats = {
  newPostingsThisWeek: DashboardPeriodMetric
  interestingPostings: DashboardInterestingPostingsMetric
  applicationsSentThisWeek: DashboardPeriodMetric
  dueFollowUps: DashboardDueFollowUpsMetric
}
