# Issue tracker: GitHub

Issues und PRDs für dieses Repository werden als GitHub Issues verwaltet. Für alle Operationen wird die `gh` CLI verwendet. Das Repository wird aus `git remote -v` abgeleitet.

## Konventionen

- **Issue erstellen:** `gh issue create --title "..." --body "..."`. Für mehrzeilige Bodies ein Heredoc verwenden.
- **Issue lesen:** `gh issue view <number> --comments`; Labels und Kommentare bei Bedarf über JSON und `jq` abfragen.
- **Issues auflisten:** `gh issue list` mit geeigneten `--state`- und `--label`-Filtern.
- **Kommentieren:** `gh issue comment <number> --body "..."`.
- **Labels ändern:** `gh issue edit <number> --add-label "..."` beziehungsweise `--remove-label "..."`.
- **Schließen:** `gh issue close <number> --comment "..."`.

Wenn ein Skill etwas „zum Issue Tracker publiziert“, wird ein GitHub Issue erstellt. Wenn ein Skill das relevante Ticket anfordert, wird es mit `gh issue view <number> --comments` geladen.

## Pull Requests als Triage-Surface

**Externe Pull Requests sind keine Request Surface für `/triage`.** Nur GitHub Issues werden triagiert.

GitHub verwendet einen gemeinsamen Nummernraum für Issues und Pull Requests. Falls ein Verweis uneindeutig ist, zuerst `gh pr view <number>` und danach `gh issue view <number>` verwenden.

## Wayfinding-Operationen

Eine Wayfinder Map ist ein einzelnes Issue mit untergeordneten Issues als Tickets.

- **Map:** Issue mit dem Label `wayfinder:map` und den Bereichen Destination, Notes, Decisions so far und Not yet specified.
- **Child Ticket:** Als GitHub Sub-Issue mit der Map verknüpft und mit `wayfinder:research`, `wayfinder:prototype`, `wayfinder:grilling` oder `wayfinder:task` gekennzeichnet. Falls Sub-Issues nicht verfügbar sind, eine Task List in der Map und `Part of #<map>` im Child verwenden.
- **Blocking:** Native GitHub Issue Dependencies sind kanonisch. Eine Abhängigkeit wird über `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>` angelegt. `<blocker-db-id>` ist die numerische Datenbank-ID aus `gh api repos/<owner>/<repo>/issues/<n> --jq .id`, nicht die Issue-Nummer. Falls Dependencies nicht verfügbar sind, `Blocked by: #<n>` im Ticketbody verwenden.
- **Frontier:** Offene Child Issues ohne offene Blocker und ohne Assignee; das erste in Map-Reihenfolge ist als Nächstes verfügbar.
- **Claim:** `gh issue edit <n> --add-assignee @me` ist der erste Schreibvorgang einer Session.
- **Resolve:** Antwort als Kommentar veröffentlichen, das Ticket schließen und einen kurzen Kontextverweis mit Link unter Decisions so far in der Map ergänzen.
