import { beforeEach, describe, expect, it, vi } from "vitest"

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/core", () => ({ invoke }))

import {
  cancelBackgroundTask,
  decodeBackgroundTaskSnapshot,
  getBackgroundTask,
  isInFlightBackgroundTask,
  isTerminalBackgroundTask,
} from "./background-tasks"

beforeEach(() => invoke.mockReset())

describe("Background Task transport", () => {
  it("decodes generic task identity, state, progress, result, error, and Diagnostics", () => {
    const snapshot = decodeBackgroundTaskSnapshot({
      taskId: "task-1",
      kind: { other: "profile_detection" },
      state: "running",
      progress: { message: "working", current: 1, total: 3 },
      result: { partial: true },
      error: null,
      diagnostics: [],
    })

    expect(snapshot.kind).toEqual({ other: "profile_detection" })
    expect(snapshot.progress).toEqual({ message: "working", current: 1, total: 3 })
  })

  it("validates get and cancel command results", async () => {
    const snapshot = {
      taskId: "task-1",
      kind: "search_run",
      state: "running",
      progress: null,
      result: null,
      error: null,
      diagnostics: [],
    }
    invoke.mockResolvedValueOnce(snapshot).mockResolvedValueOnce({
      ...snapshot,
      state: "cancelling",
    })

    await expect(getBackgroundTask("task-1")).resolves.toMatchObject({ state: "running" })
    await expect(cancelBackgroundTask("task-1")).resolves.toMatchObject({ state: "cancelling" })
    expect(invoke.mock.calls).toEqual([
      ["get_background_task", { taskId: "task-1" }],
      ["cancel_background_task", { taskId: "task-1" }],
    ])
  })

  it.each([
    ["unknown state", { state: "paused" }],
    ["malformed kind", { kind: { other: 1 } }],
    ["malformed progress", { progress: { message: "working", current: -1, total: 3 } }],
    ["malformed error", { error: 1 }],
    ["malformed Diagnostics", { diagnostics: "invalid" }],
    ["missing result", { result: undefined }],
  ])("rejects %s", (_case, override) => {
    expect(() => decodeBackgroundTaskSnapshot({
      taskId: "task-1",
      kind: "search_run",
      state: "queued",
      progress: null,
      result: null,
      error: null,
      diagnostics: [],
      ...override,
    })).toThrow(/invalid Background Task/i)
  })

  it("classifies shared polling states", () => {
    expect(isInFlightBackgroundTask({ state: "cancelling" })).toBe(true)
    expect(isInFlightBackgroundTask({ state: "succeeded" })).toBe(false)
    expect(isTerminalBackgroundTask({ state: "cancelled" })).toBe(true)
    expect(isTerminalBackgroundTask({ state: "running" })).toBe(false)
  })
})
