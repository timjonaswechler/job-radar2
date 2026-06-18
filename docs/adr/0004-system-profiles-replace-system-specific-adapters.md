---
status: superseded by ADR-0006 and ADR-0007
---

# Replace system-specific adapters with declarative system profiles

This decision's enduring part remains: adapters are technical runtimes, not recruiting-system identities. The specific Systemprofil model has been superseded by Quellenprofile with profile-defined access paths, and profile/source knowledge is moving out of SQLite-owned domain tables into authoritative JSON documents.
