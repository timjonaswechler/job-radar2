# Use UUIDv7 for domain object identifiers

Job Radar uses UUIDv7 identifiers for postings, applications, findings, search queries, job sources, search runs, and reminders. IDs must remain stable across JSON backup and restore so deep links continue to work, while UUIDv7 keeps identifiers globally unique and roughly time-sortable for local storage and possible later service mode.
