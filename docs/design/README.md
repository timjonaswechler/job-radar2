# Design-Dokumentation

Diese Dokumente beschreiben, **für wen** Job Radar gestaltet wird, **welche Informationen** in der UI sichtbar sein sollen und **wie** Agenten UI-Änderungen konsistent umsetzen.

Vor UI-Arbeit zuerst lesen:

1. [`product-audience.md`](product-audience.md) — Zielgruppe, Nutzermodi und Produktperspektive
2. [`information-architecture.md`](information-architecture.md) — allgemeine Screen- und Navigationsmuster
3. [`ui-data-rules.md`](ui-data-rules.md) — welche Daten sichtbar sind und welche intern bleiben
4. [`domain-ui-contract.md`](domain-ui-contract.md) — wie UI-Zustände fachlich korrekt aus Backend-/Domainzuständen abgeleitet werden
5. [`visual-direction.md`](visual-direction.md) — visuelle Richtung aus Screenshots und Video-Inspiration
6. [`inspiration.md`](inspiration.md) — konkrete Referenzen als übertragbare Prinzipien, nicht als Kopiervorlage

Ergänzende Domänen-/UI-Mappings:

- [`job-posting-queues.md`](job-posting-queues.md) — Quelle der Wahrheit für Stellenanzeigen-Queues, Read-State-Indikatoren und Counts

## Grundsatz

Job Radar ist eine ruhige, datengetriebene Arbeitsoberfläche. Die UI zeigt nicht automatisch alles, was Backend, Agenten oder Registry intern wissen. Sie bleibt aber fachlich treu zu den Backend- und Domainzuständen. Sie zeigt zuerst das, was Nutzer:innen hilft:

- zu verstehen, was passiert ist,
- zu entscheiden, was als Nächstes zu tun ist,
- Vertrauen in automatische Abläufe aufzubauen,
- Probleme verständlich zu beheben.

Interne Details bleiben verfügbar, aber progressiv verborgen: Detailpanel, erweiterte Ansicht, Debug-Abschnitt oder Rohdatenansicht.
