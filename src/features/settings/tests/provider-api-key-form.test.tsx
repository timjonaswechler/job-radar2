// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";

import { ProviderApiKeyForm } from "@/features/settings/agent-provider-settings";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

afterEach(cleanup);

test("submits an API key once and clears the sensitive input", async () => {
  const user = userEvent.setup();
  const syntheticSecret = ["synthetic", "api", "key"].join("-");
  let submissionCount = 0;
  let receivedProvider = "";
  let receivedExpectedKey = false;

  render(
    <ProviderApiKeyForm
      providerId="provider-one"
      activeAuthentication={null}
      busy={false}
      onSubmitApiKey={async (providerId, apiKey) => {
        submissionCount += 1;
        receivedProvider = providerId;
        receivedExpectedKey = apiKey === syntheticSecret;
      }}
    />,
  );

  const input = screen.getByLabelText("settings.agents.apiKey.title");
  await user.type(input, syntheticSecret);
  await user.click(
    screen.getByRole("button", {
      name: "settings.agents.actions.saveApiKey",
    }),
  );

  await waitFor(() => expect(submissionCount).toBe(1));
  expect(receivedProvider).toBe("provider-one");
  expect(receivedExpectedKey).toBe(true);
  expect(input).toHaveValue("");
});
