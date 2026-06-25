# Stellenanzeigen-Queues

Dieses Dokument hält die fachliche Quelle der Wahrheit für die mailbox-artige Stellenanzeigen-Navigation fest. Die UI darf Counts und Listen getrennt laden, muss aber dieselben Queue-Prädikate verwenden.

Die Queue-Namen `Inbox`, `Interessant`, `Bewerbung vorbereiten`, `Beworben / Warten`, `Archiv` und `Alle Anzeigen` sind für diesen ersten Slice fachlich finalisiert. `Neu` und `Gelesen` sind Read-State-Indikatoren in der Liste, keine eigenen Menüpunkte oder Inbox-Unterqueues.

## Sichtbarkeitsmatrix

```txt
UI-Feld | Nutzerlabel | Quelle der Wahrheit | Herleitung | Disclosure-Level
Queue | Inbox | interestState, applicationState, terminale Archivzustände | nicht archiviert + interestState: undecided + applicationState: not_applied | Navigation/Liste
Read-State | Neu | readState | readState: unread | Zeile
Read-State | Gelesen | readState | readState: read | Zeile
Queue | Interessant | interestState, preparationState, applicationState | interestState: interested + preparationState: not_started + applicationState: not_applied | Navigation/Liste
Queue | Bewerbung vorbereiten | interestState, preparationState, applicationState | interestState: interested + preparationState: in_progress | ready + applicationState: not_applied | Navigation/Liste
Queue | Beworben / Warten | applicationState, Archiv-Klassifikation | nicht archiviert + applicationState: submitted | in_process | Navigation/Liste
Queue | Archiv | interestState, applicationState | interestState: dismissed OR applicationState: rejected_by_company | withdrawn_by_me | accepted | Navigation/Liste
Queue | Alle Anzeigen | persistierte Stellenanzeigen | alle gespeicherten JobPosting-Datensätze | Navigation/Liste
Count | Zahl rechts an Queue | dieselben Prädikate wie Queue/List-Command | Backend-Count-Command, nicht aus der aktuellen UI-Liste geraten | Navigation
Unread marker | Punkt/Badge in Zeile | readState | unread hebt die Zeile wie ungelesene Mail hervor | Zeile
```

## Exklusive UI-Klassifikation

Die Backend-Felder erlauben technisch Kombinationen wie `interestState: undecided` zusammen mit `applicationState: submitted`. Für die UI werden die linken Queues exklusiv klassifiziert, damit eine Anzeige nicht gleichzeitig in Inbox und Bewerbungsprozess erscheint.

Priorität:

1. `Archiv` bei `interestState: dismissed` oder terminalem `applicationState`.
2. `Beworben / Warten` bei `applicationState: submitted | in_process`.
3. `Inbox` bei `interestState: undecided` und `applicationState: not_applied`.
4. `Interessant` bei `interestState: interested`, `preparationState: not_started`, `applicationState: not_applied`.
5. `Bewerbung vorbereiten` bei `interestState: interested`, `preparationState: in_progress | ready`, `applicationState: not_applied`.
6. `Alle Anzeigen` bleibt die vollständige Bestandsansicht.

Diese Priorität ist in Frontend-Workflow-Logik und Backend-Queue-Commands nachzuziehen, nicht in einzelnen UI-Komponenten neu zu erfinden.

## Inbox-Verhalten

- `Inbox` zeigt eine flache Liste aller Anzeigen, die noch eine Entscheidung brauchen.
- `readState: unread` und `readState: read` werden wie ungelesene/gelesene Mail direkt in der Zeile markiert.
- Es gibt keine Inbox-Untermenüs und keine collapsible Inbox-Gruppen.
- Wenn später Sortierung nötig wird, kann `unread` innerhalb der flachen Inbox-Liste priorisiert werden, ohne daraus eine neue Navigationsebene zu machen.

## Datenfluss

- Sidebar-Counts werden leichtgewichtig über `get_job_posting_queue_counts` geladen.
- Die aktive Liste wird queue-spezifisch über `list_job_postings_for_queue` geladen.
- Counts und Listen müssen nicht aus demselben Frontend-Array stammen, aber aus denselben fachlichen Prädikaten.
