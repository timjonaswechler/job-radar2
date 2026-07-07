# Replace scattered profile checks with a unified Profile Verification Check

Job Radar will expose Source Profile verification as one coherent Profile Verification Check that produces a Verification Report by orchestrating schema validation, registry validation, Profile Compiler validation, fixture evidence resolution, fixture execution, and Effective Verification State derivation. Because the app is still pre-production, existing scattered profile-check logic, commands, diagnostics, and UI paths may be cleanly replaced instead of wrapped, aliased, or preserved for compatibility; this keeps the Source Profile verification model explicit and prevents legacy dev-era semantics from becoming part of the product contract.

## Consequences

- Profile verification has one user- and agent-facing entry point: `Prüfen` / Profile Verification Check.
- Verification-specific diagnostics belong to the verification system rather than being preserved under older categories for compatibility.
- Tests and UI should be updated to the new model instead of maintaining parallel old and new behavior.
