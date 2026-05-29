# Use local SQLite as the phase-one data store

Job Radar starts as a single-user desktop application, so phase one stores applications, job postings, findings, search configuration, and search history in a local SQLite database. A later service mode may move the same domain model behind a service boundary, but the first implementation optimizes for local ownership, offline use, and low operational overhead instead of multi-user hosting.
