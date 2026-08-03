// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  cancelBackgroundTask: vi.fn(),
  getBackgroundTask: vi.fn(),
  runSearchRequest: vi.fn(),
}));
const notifications = vi.hoisted(() => ({
  error: vi.fn(),
  info: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
}));

vi.mock("@/lib/api/background-tasks", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/lib/api/background-tasks")>(),
  cancelBackgroundTask: api.cancelBackgroundTask,
  getBackgroundTask: api.getBackgroundTask,
}));
vi.mock("@/lib/api/search-runs", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/lib/api/search-runs")>(),
  runSearchRequest: api.runSearchRequest,
}));
vi.mock("sonner", () => ({ toast: notifications }));

import { useSearchRun } from "@/features/search-requests/use-search-run";
import type { BackgroundTaskSnapshot } from "@/lib/api/background-tasks";

beforeEach(() => {
  vi.clearAllMocks();
  api.cancelBackgroundTask.mockReset();
  api.getBackgroundTask.mockReset();
  api.runSearchRequest.mockReset();
  notifications.error.mockReset();
  notifications.info.mockReset();
  notifications.success.mockReset();
  notifications.warning.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useSearchRun", () => {
  it("admits only one start so overlapping calls cannot orphan a task", async () => {
    const first = deferred<BackgroundTaskSnapshot>();
    api.runSearchRequest.mockReturnValueOnce(first.promise);
    const { result } = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));

    act(() => {
      void result.current.start(1, "First request");
      void result.current.start(2, "Second request");
    });
    await act(async () => {
      first.resolve(task("task-1", "queued"));
      await first.promise;
    });

    expect(api.runSearchRequest).toHaveBeenCalledTimes(1);
    expect(api.runSearchRequest).toHaveBeenCalledWith(1);
    expect(result.current.operation).toMatchObject({
      status: "active",
      searchRequestId: 1,
      task: { taskId: "task-1" },
    });
  });

  it("polls after one second and completes only the matching operation", async () => {
    vi.useFakeTimers();
    const onCompleted = vi.fn();
    api.runSearchRequest.mockResolvedValueOnce(task("task-1", "queued"));
    api.getBackgroundTask.mockResolvedValueOnce(task(
      "task-1",
      "succeeded",
      outcome(1, "completed"),
    ));
    const { result } = renderHook(() => useSearchRun({ onCompleted }));

    await act(async () => {
      await result.current.start(1, "First request");
    });
    expect(api.getBackgroundTask).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(999);
    });
    expect(api.getBackgroundTask).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });

    expect(api.getBackgroundTask).toHaveBeenCalledWith("task-1");
    expect(result.current.operation).toMatchObject({
      status: "terminal",
      searchRequestId: 1,
      outcome: { status: "completed" },
    });
    expect(onCompleted).toHaveBeenCalledTimes(1);
  });

  it("rejects a terminal Outcome for a different Search Request", async () => {
    const onCompleted = vi.fn();
    api.runSearchRequest.mockResolvedValueOnce(task(
      "task-1",
      "succeeded",
      outcome(99, "completed"),
    ));
    const { result } = renderHook(() => useSearchRun({ onCompleted }));

    await act(async () => {
      await result.current.start(1, "First request");
    });

    expect(result.current.operation).toMatchObject({
      status: "interrupted",
      searchRequestId: 1,
      task: { taskId: "task-1" },
    });
    expect(onCompleted).not.toHaveBeenCalled();
  });

  it("ignores a stale poll response after a newer start", async () => {
    vi.useFakeTimers();
    const stalePoll = deferred<BackgroundTaskSnapshot>();
    api.runSearchRequest
      .mockResolvedValueOnce(task("task-1", "running"))
      .mockResolvedValueOnce(task("task-2", "queued"));
    api.getBackgroundTask.mockReturnValueOnce(stalePoll.promise);
    api.cancelBackgroundTask.mockResolvedValueOnce(task(
      "task-1",
      "cancelled",
      outcome(1, "cancelled"),
    ));
    const { result } = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));

    await act(async () => {
      await result.current.start(1, "First request");
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    await act(async () => {
      await result.current.cancel();
      await result.current.start(2, "Second request");
    });
    await act(async () => {
      stalePoll.resolve(task("task-1", "succeeded", outcome(1, "completed")));
      await stalePoll.promise;
    });

    expect(result.current.operation).toMatchObject({
      status: "active",
      searchRequestId: 2,
      task: { taskId: "task-2" },
    });
  });

  it("does not replace the current task with a mismatched poll response", async () => {
    vi.useFakeTimers();
    api.runSearchRequest.mockResolvedValueOnce(task("task-1", "running"));
    api.getBackgroundTask.mockResolvedValueOnce(task("task-other", "running"));
    const { result } = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));

    await act(async () => {
      await result.current.start(1, "First request");
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(result.current.operation).toMatchObject({
      status: "interrupted",
      task: { taskId: "task-1" },
    });
  });

  it("accepts immediate queued-task cancellation without a domain Outcome", async () => {
    const onCompleted = vi.fn();
    api.runSearchRequest.mockResolvedValueOnce(task("task-1", "queued"));
    api.cancelBackgroundTask.mockResolvedValueOnce(task("task-1", "cancelled"));
    const { result } = renderHook(() => useSearchRun({ onCompleted }));

    await act(async () => {
      await result.current.start(1, "First request");
      await result.current.cancel();
    });

    expect(result.current.operation).toMatchObject({
      status: "terminal",
      task: { state: "cancelled" },
      outcome: null,
    });
    expect(onCompleted).toHaveBeenCalledTimes(1);
  });

  it("keeps polling while running-task cancellation is pending", async () => {
    api.runSearchRequest.mockResolvedValueOnce(task("task-1", "running"));
    api.cancelBackgroundTask.mockResolvedValueOnce(task("task-1", "cancelling"));
    const { result } = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));

    await act(async () => {
      await result.current.start(1, "First request");
      await result.current.cancel();
    });

    expect(api.cancelBackgroundTask).toHaveBeenCalledWith("task-1");
    expect(result.current.operation).toMatchObject({
      status: "active",
      task: { state: "cancelling" },
      cancelling: false,
    });
  });

  it("does not let an older cancel response regress a newer poll state", async () => {
    vi.useFakeTimers();
    const cancelResponse = deferred<BackgroundTaskSnapshot>();
    api.runSearchRequest.mockResolvedValueOnce(task("task-1", "running"));
    api.cancelBackgroundTask.mockReturnValueOnce(cancelResponse.promise);
    api.getBackgroundTask.mockResolvedValueOnce(task("task-1", "cancelling"));
    const { result } = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));

    await act(async () => {
      await result.current.start(1, "First request");
    });
    act(() => {
      void result.current.cancel();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    await act(async () => {
      cancelResponse.resolve(task("task-other", "running"));
      await cancelResponse.promise;
    });

    expect(result.current.operation).toMatchObject({
      status: "active",
      task: { state: "cancelling" },
    });
  });

  it("keeps a committed successful Outcome when an older cancel response arrives", async () => {
    vi.useFakeTimers();
    const cancelResponse = deferred<BackgroundTaskSnapshot>();
    const onCompleted = vi.fn();
    api.runSearchRequest.mockResolvedValueOnce(task("task-1", "running"));
    api.cancelBackgroundTask.mockReturnValueOnce(cancelResponse.promise);
    api.getBackgroundTask.mockResolvedValueOnce(task(
      "task-1",
      "succeeded",
      outcome(1, "completed"),
    ));
    const { result } = renderHook(() => useSearchRun({ onCompleted }));

    await act(async () => {
      await result.current.start(1, "First request");
    });
    act(() => {
      void result.current.cancel();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(api.getBackgroundTask).toHaveBeenCalledWith("task-1");
    expect(result.current.operation).toMatchObject({
      status: "terminal",
      task: { state: "succeeded" },
    });
    await act(async () => {
      cancelResponse.resolve(task("task-1", "cancelled"));
      await cancelResponse.promise;
    });

    expect(result.current.operation).toMatchObject({
      status: "terminal",
      task: { state: "succeeded" },
      outcome: { status: "completed" },
    });
    expect(onCompleted).toHaveBeenCalledTimes(1);
  });

  it.each([
    "completed",
    "completed_with_errors",
    "failed",
    "cancelled",
  ] as const)("accepts the authoritative %s domain Outcome", async (status) => {
    const onCompleted = vi.fn();
    api.runSearchRequest.mockResolvedValueOnce(task(
      "task-1",
      status === "cancelled" ? "cancelled" : "succeeded",
      outcome(1, status),
    ));
    const { result } = renderHook(() => useSearchRun({ onCompleted }));

    await act(async () => {
      await result.current.start(1, "First request");
    });

    expect(result.current.operation).toMatchObject({
      status: "terminal",
      outcome: { status },
    });
    expect(onCompleted).toHaveBeenCalledTimes(1);
  });

  it("surfaces a terminal Background Task failure and refreshes once", async () => {
    const onCompleted = vi.fn();
    api.runSearchRequest.mockResolvedValueOnce({
      ...task("task-1", "failed"),
      error: "worker failed",
    });
    const { result } = renderHook(() => useSearchRun({ onCompleted }));

    await act(async () => {
      await result.current.start(1, "First request");
    });

    expect(result.current.operation).toMatchObject({
      status: "terminal",
      task: { state: "failed" },
      error: "worker failed",
    });
    expect(onCompleted).toHaveBeenCalledTimes(1);
  });

  it("surfaces start, poll, and cancel failures without stale replacement", async () => {
    api.runSearchRequest.mockRejectedValueOnce(new Error("start failed"));
    const first = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));
    await act(async () => {
      await first.result.current.start(1, "First request");
    });
    expect(first.result.current.operation).toMatchObject({
      status: "interrupted",
      error: "start failed",
    });
    first.unmount();

    vi.useFakeTimers();
    api.runSearchRequest.mockResolvedValueOnce(task("task-2", "running"));
    api.getBackgroundTask.mockRejectedValueOnce(new Error("poll failed"));
    api.cancelBackgroundTask.mockRejectedValueOnce(new Error("cancel failed"));
    const second = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));
    await act(async () => {
      await second.result.current.start(2, "Second request");
    });
    await act(async () => {
      await second.result.current.cancel();
    });
    expect(second.result.current.operation).toMatchObject({
      status: "active",
      cancelling: false,
      error: "cancel failed",
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(api.getBackgroundTask).toHaveBeenCalledWith("task-2");
    expect(second.result.current.operation).toMatchObject({
      status: "interrupted",
      error: "poll failed",
    });
  });

  it("ignores a pending start response after unmount", async () => {
    const pendingStart = deferred<BackgroundTaskSnapshot>();
    const onCompleted = vi.fn();
    api.runSearchRequest.mockReturnValueOnce(pendingStart.promise);
    const { result, unmount } = renderHook(() => useSearchRun({ onCompleted }));

    act(() => {
      void result.current.start(1, "First request");
    });
    unmount();
    await act(async () => {
      pendingStart.resolve(task("task-1", "succeeded", outcome(1, "completed")));
      await pendingStart.promise;
    });

    expect(onCompleted).not.toHaveBeenCalled();
  });

  it("suppresses a pending start rejection after unmount", async () => {
    const pendingStart = deferred<BackgroundTaskSnapshot>();
    api.runSearchRequest.mockReturnValueOnce(pendingStart.promise);
    const { result, unmount } = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));

    act(() => {
      void result.current.start(1, "First request");
    });
    unmount();
    await act(async () => {
      pendingStart.reject(new Error("late failure"));
      await expect(pendingStart.promise).rejects.toThrow("late failure");
    });

    expect(notifications.error).not.toHaveBeenCalled();
  });

  it("cleans up polling on unmount and does not recover an operation", async () => {
    vi.useFakeTimers();
    api.runSearchRequest.mockResolvedValueOnce(task("task-1", "running"));
    const { result, unmount } = renderHook(() => useSearchRun({ onCompleted: vi.fn() }));
    await act(async () => {
      await result.current.start(1, "First request");
    });

    unmount();
    await vi.advanceTimersByTimeAsync(2000);

    expect(api.getBackgroundTask).not.toHaveBeenCalled();
  });
});

function task(
  taskId: string,
  state: BackgroundTaskSnapshot["state"],
  result: unknown = null,
): BackgroundTaskSnapshot {
  return {
    taskId,
    kind: "search_run",
    state,
    progress: null,
    result,
    error: null,
    diagnostics: [],
  };
}

function outcome(
  searchRequestId: number,
  status: "completed" | "completed_with_errors" | "failed" | "cancelled",
) {
  return {
    searchRequestId,
    status,
    generatedAt: "2026-08-02T12:00:00Z",
    diagnostics: [],
    sourceRuns: [],
    matchedPostingCount: 0,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}
