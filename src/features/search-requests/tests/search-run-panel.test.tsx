// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SearchRunPanel } from "@/features/search-requests/components/search-run-panel";
import type { SearchRunOperation } from "@/features/search-requests/use-search-run";

vi.mock("sonner", () => ({
  toast: { error: vi.fn(), info: vi.fn(), success: vi.fn(), warning: vi.fn() },
}));

describe("SearchRunPanel", () => {
  it("shows bounded task Diagnostics and preserves Source Run outcomes", () => {
    const operation: Exclude<SearchRunOperation, { status: "idle" }> = {
      status: "terminal",
      searchRequestId: 7,
      generation: 1,
      task: {
        taskId: "task-7",
        kind: "search_run",
        state: "succeeded",
        progress: null,
        result: null,
        error: null,
        diagnostics: [{
          category: "runtime",
          code: "task_warning",
          message: "Task warning remains observable",
          severity: "warning",
          path: "/task",
        }],
      },
      outcome: {
        searchRequestId: 7,
        status: "completed_with_errors",
        generatedAt: "2026-08-02T12:00:00Z",
        diagnostics: [],
        sourceRuns: [{
          sourceKey: "fixture_source",
          sourceName: "Fixture Source",
          status: "failed",
          resolution: null,
          diagnostics: [],
          error: "Source failed",
        }],
        matchedPostingCount: 0,
      },
      error: null,
    };

    render(<SearchRunPanel row={null} operation={operation} onCancel={vi.fn()} />);

    expect(screen.getByText("Background-Task-Diagnostics (1)")).toBeVisible();
    expect(screen.getByText("task_warning")).toBeVisible();
    expect(screen.getByText("Task warning remains observable")).toBeVisible();
    expect(screen.getByText("Fixture Source")).toBeVisible();
    expect(screen.getByText("Source failed")).toBeVisible();
  });
});
