# Source Registry cleanup progress

- Removed stale DB-owned source/profile code paths and tests.
- Source and source-profile knowledge is loaded from JSON registry documents under `source-profiles/` and `sources/`.
- Legacy system/browser profile terminology is documented only as historical/superseded.
- Validation is tracked in the worker report for issue #43.
