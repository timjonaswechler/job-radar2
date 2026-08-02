import { invoke } from "@tauri-apps/api/core"

import type { StructuredDiagnostic } from "@/lib/api/sources"
import {
  isArrayOf,
  isNonNegativeSafeInteger,
  isRecord,
  isStructuredDiagnostic,
} from "./wire"

export type BackgroundTaskState =
  | "queued"
  | "running"
  | "cancelling"
  | "succeeded"
  | "failed"
  | "cancelled"

export type BackgroundTaskKind = "search_run" | { other: string }

export type BackgroundTaskProgress = {
  message: string
  current: number | null
  total: number | null
}

export type BackgroundTaskSnapshot = {
  taskId: string
  kind: BackgroundTaskKind
  state: BackgroundTaskState
  progress: BackgroundTaskProgress | null
  result: unknown | null
  error: string | null
  diagnostics: StructuredDiagnostic[]
}

type TaskState = Pick<BackgroundTaskSnapshot, "state">

export function decodeBackgroundTaskSnapshot(value: unknown): BackgroundTaskSnapshot {
  if (
    !isRecord(value) ||
    typeof value.taskId !== "string" ||
    value.taskId.length === 0 ||
    !isBackgroundTaskKind(value.kind) ||
    !isBackgroundTaskState(value.state) ||
    !(value.progress === null || isBackgroundTaskProgress(value.progress)) ||
    value.result === undefined ||
    !(value.error === null || typeof value.error === "string") ||
    !isArrayOf(value.diagnostics, isStructuredDiagnostic)
  ) {
    throw new Error("Invalid Background Task response shape.")
  }

  return {
    taskId: value.taskId,
    kind: value.kind,
    state: value.state,
    progress: value.progress,
    result: value.result,
    error: value.error,
    diagnostics: value.diagnostics,
  }
}

export async function getBackgroundTask(taskId: string) {
  return decodeBackgroundTaskSnapshot(
    await invoke<unknown>("get_background_task", { taskId }),
  )
}

export async function cancelBackgroundTask(taskId: string) {
  return decodeBackgroundTaskSnapshot(
    await invoke<unknown>("cancel_background_task", { taskId }),
  )
}

export function isInFlightBackgroundTask(task: TaskState | null): boolean {
  return task?.state === "queued" || task?.state === "running" || task?.state === "cancelling"
}

export function isTerminalBackgroundTask(task: TaskState): boolean {
  return task.state === "succeeded" || task.state === "failed" || task.state === "cancelled"
}

function isBackgroundTaskKind(value: unknown): value is BackgroundTaskKind {
  return value === "search_run" || (
    isRecord(value) &&
    typeof value.other === "string" &&
    Object.keys(value).length === 1
  )
}

function isBackgroundTaskState(value: unknown): value is BackgroundTaskState {
  return [
    "queued",
    "running",
    "cancelling",
    "succeeded",
    "failed",
    "cancelled",
  ].includes(String(value))
}

function isBackgroundTaskProgress(value: unknown): value is BackgroundTaskProgress {
  return (
    isRecord(value) &&
    typeof value.message === "string" &&
    (value.current === null || isNonNegativeSafeInteger(value.current)) &&
    (value.total === null || isNonNegativeSafeInteger(value.total))
  )
}
