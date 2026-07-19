# Domain Docs

Job Radar verwendet ein Single-Context-Layout. `CONTEXT.md` und `docs/adr/` im Repository-Root gelten für Frontend und Backend.

## Vor der Arbeit lesen

- `CONTEXT.md` für die kanonische Domain-Sprache.
- Die ADRs unter `docs/adr/`, die den zu bearbeitenden Bereich betreffen.
- Für Source/Profile-DSL-Arbeit zusätzlich die in `AGENTS.md` genannten PRDs und ADRs.

Falls eine dieser Dateien nicht existiert, wird stillschweigend weitergearbeitet. Domain-Dokumentation wird durch `/domain-modeling` bedarfsgerecht ergänzt, nicht vorsorglich erzeugt.

## Glossar verwenden

In Issues, Plänen, Tests und Code werden die Begriffe aus `CONTEXT.md` verwendet. Synonyme, die das Glossar ausdrücklich vermeidet, werden nicht neu eingeführt.

Fehlt ein benötigter Begriff, ist zu prüfen, ob eine ungebräuchliche Bezeichnung erfunden wird oder eine echte Lücke für `/domain-modeling` vorliegt.

## ADR-Konflikte sichtbar machen

Widerspricht eine geplante Änderung einem bestehenden ADR, wird der Konflikt ausdrücklich benannt, statt das ADR stillschweigend zu überschreiben.
