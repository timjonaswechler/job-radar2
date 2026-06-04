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

export type PostingStatus =
  | "new"
  | "interesting"
  | "review_later"
  | "hidden"
  | "converted_to_application"

export type WorkModel = "remote" | "hybrid" | "on_site" | "unknown"

export type DashboardPosting = {
  id: string
  title: string
  company: string
  primaryLocation: string | null
  region: string | null
  workModel: WorkModel
  status: PostingStatus
  descriptionExcerpt: string
  createdAt: string
  updatedAt: string
  findingCount: number
  lastFoundAt: string | null
  latestResultUrl: string | null
  latestSourceName: string | null
}
