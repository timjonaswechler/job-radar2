# macOS OpenAI subscription live smoke

Issue [#190](https://github.com/timjonaswechler/job-radar2/issues/190) verified the first complete Agent Conversation vertical slice on macOS on 2026-07-15. The operator ran the explicit `npm run agent:debug` path against an OpenAI ChatGPT/Codex subscription.

Only sanitized outcomes are recorded here. No authorization URL, callback value, access or refresh token, account identifier, email address, request header, credential path, prompt content, model response, or raw provider payload was retained in the repository or issue.

## Results

| Check | Outcome |
| --- | --- |
| Harness starts through `npm run agent:debug` | Passed |
| Unauthenticated prompt returns the stable authentication failure category | Passed |
| Browser PKCE login completes and status becomes `configured` | Passed |
| Multiple conversational turns stream text successfully | Passed |
| Provider-approved reasoning content streams separately | Passed |
| `/settings` changes only the session-local Reasoning Level | Passed |
| Default and alternate pinned models are listed in a numbered `/model` menu | Passed |
| Two alternate Agent Models can be selected and used for subsequent turns | Passed |
| Process restart retains valid authentication without another login | Passed |
| A conversational turn succeeds after restart | Passed |
| `Ctrl+C` exits the entire harness | Passed |
| `/logout` removes the local credential and reports `not configured` | Passed |
| `/quit` exits cleanly | Passed |

The initial and restarted runs completed without a reproducible compatibility failure. No correction ticket was required.

## Safety procedure

Live authentication and account-linked values remain local to the operator's terminal and protected application-data storage. Reports must contain only the value-free authentication state, selected public Agent Model identifiers, stable error categories, and pass/fail outcomes. Run the repository safeguard before committing this record:

```bash
npm run test:agent-credential-safety
npm run check:agent-credentials
```
