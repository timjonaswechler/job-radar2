# Job Radar documentation

Die Dokumentation ist nach Zielgruppe in zwei Bereiche getrennt:

- **[Job Radar verwenden](user/README.md):** veröffentlichbare Dokumentation für Endanwender und die spätere In-App-Hilfe.
- **[Job Radar entwickeln](development/README.md):** technische, architektonische und projektinterne Dokumentation für Contributors und Agents; die unvollständige Agent-Assistance-Grundlage ist in [`ADR 0017`](development/adr/0017-agent-assistance-foundation.md) dokumentiert.

Der Produktüberblick und die lokale Einrichtung bleiben in der Repository-[`README.md`](../README.md).

## Ablageregeln

Neue Dokumente werden nach ihrer primären Zielgruppe abgelegt:

1. Anleitungen, Konzepte und Troubleshooting für Anwender gehören nach [`user/`](user/).
2. Entwicklungsabläufe, technische Verträge, Architekturentscheidungen, PRDs, Referenzen und interne Projektprozesse gehören nach [`development/`](development/).
3. Dient ein Dokument mehreren Zielgruppen, gibt es genau eine kanonische Fassung. Der andere Bereich verlinkt darauf, statt Inhalte zu duplizieren.

Zeitgebundene Untersuchungen gehören nach [`development/research/`](development/research/). Nicht jede gepflegte Datei gehört unter `docs/`: aktive Übergaben bleiben in [`handoff/`](../handoff/), ausführbare Werkzeuge in [`scripts/`](../scripts/) und das kanonische Domain-Vokabular in [`CONTEXT.md`](../CONTEXT.md).

Nach Änderungen an der Dokumentation muss `npm run check:markdown-links` oder `just docs-check` ausgeführt werden. Der Self-Test des Link-Checkers läuft mit `npm run test:markdown-links`.
