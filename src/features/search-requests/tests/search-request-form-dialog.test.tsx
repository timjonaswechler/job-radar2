// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";

import { SearchRequestFormDialog } from "@/features/search-requests/components/search-request-form-dialog";
import type { SearchRequest } from "@/lib/api/search-requests";
import type { InstalledSource } from "@/lib/api/sources";

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
vi.stubGlobal("ResizeObserver", ResizeObserverStub);
Element.prototype.getAnimations = () => [];

afterEach(cleanup);

test("duplicate Source warning follows the edited form and clears after remediation", async () => {
  const user = userEvent.setup();
  const submittedSourceKeys: string[][] = [];

  render(
    <SearchRequestFormDialog
      open
      request={duplicateDraft()}
      sources={[installedSource()]}
      defaultSearchRadiusKm={42}
      onOpenChange={() => {}}
      onSubmit={async (input) => {
        submittedSourceKeys.push(input.sourceKeys);
      }}
    />,
  );

  expect(await screen.findAllByText(/Source Key ist doppelt ausgewählt/)).toHaveLength(1);
  expect(screen.queryByText(/Source key duplicates/)).not.toBeInTheDocument();

  const sourceCheckbox = screen.getByRole("checkbox", { name: /Fixture Source/ });
  await user.click(sourceCheckbox);
  await user.click(sourceCheckbox);

  await waitFor(() => {
    expect(screen.queryByText(/Source Key ist doppelt ausgewählt/)).not.toBeInTheDocument();
  });
  await user.click(screen.getByRole("button", { name: "Änderungen speichern" }));

  await waitFor(() => {
    expect(submittedSourceKeys).toEqual([["fixture_source"]]);
  });
});

function duplicateDraft(): SearchRequest {
  return {
    id: 1,
    status: "draft",
    includeRules: [{ target: "title", kind: "text", value: "Physik" }],
    excludeRules: [],
    locations: [],
    radiusKm: null,
    sourceKeys: ["fixture_source", "fixture_source"],
    validationIssues: [{
      code: "duplicate_source_key",
      path: "/sourceKeys/1",
      message: "Source key duplicates /sourceKeys/0; remove the duplicate entry.",
    }],
    lastRunAt: null,
    lastRunStatus: null,
    lastRunError: null,
    createdAt: "2026-07-09T00:00:00Z",
    updatedAt: "2026-07-09T00:00:00Z",
  };
}

function installedSource(): InstalledSource {
  return {
    origin: "custom",
    fileName: "fixture_source.json",
    document: {
      schemaVersion: 3,
      key: "fixture_source",
      name: "Fixture Source",
      status: "active",
      sourceConfig: {},
      accessPaths: [],
      selectedAccessPath: {
        type: "profile_access_path",
        profileKey: "fixture_profile",
        pathKey: "fixture_path",
      },
    },
    validationState: {
      sourceKey: "fixture_source",
      state: "valid",
      canCompile: true,
      canExecute: true,
      diagnostics: [],
    },
  };
}
