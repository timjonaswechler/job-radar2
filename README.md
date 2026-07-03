# Job Radar

Job Radar ist ein Desktop-Werkzeug für Bewerber:innen, die ihre Jobsuche automatisierter und übersichtlicher machen wollen: Du beschreibst einmal, **wo** relevante Stellen auftauchen können, formulierst **wonach** du suchst, und lässt die Suche anschließend als nachvollziehbaren Suchlauf ausführen.

Die Idee dahinter ist simpel: Jobs liegen heute verstreut über Jobportale, Recruiting-Systeme und einzelne Karriere-Websites. Job Radar soll diese Quellen für dich im Blick behalten, Treffer vergleichbar zusammenführen und sichtbar machen, welche Quelle was geliefert hat — ohne dass jede Suche wieder bei null beginnt.

## Für wen ist das gedacht?

Job Radar richtet sich an Bewerber:innen, die ihre Jobsuche nicht in zehn Browser-Tabs, gespeicherten Links und immer gleichen Suchformularen verlieren wollen.

Es hilft dabei, bestimmte Rollen, Orte oder Branchen im Blick zu behalten, Suchläufe zu wiederholen und neue Treffer nachvollziehbar einzuordnen.

Das Projekt ist aktuell ein früher Prototyp. Die Grundbegriffe, die lokale App-Basis, die Quellen-Registry und erste Suchlauf-Pfade existieren; die komfortable UI für den kompletten Alltag wächst schrittweise nach.

## Die Kernidee

Job Radar trennt bewusst drei Dinge, die in vielen Job-Suchen vermischt werden:

1. **Quellen** — Orte, an denen Stellen gefunden werden können. Zum Beispiel ein Jobportal, eine Firmen-Karriere-Seite oder ein Recruiting-System.
2. **Suchanfragen** — deine Suchabsicht: Begriffe, Ausschlüsse, Orte, Radius und die ausgewählten Quellen.
3. **Suchläufe** — konkrete Ausführungen zu einem Zeitpunkt. Ein Suchlauf zeigt, welche Quellen funktioniert haben, welche teilweise fehlgeschlagen sind und welche Stellen am Ende übrig bleiben.

Diese Trennung macht die Suche wiederholbar: Eine Quelle kann für viele Suchanfragen genutzt werden; eine Suchanfrage kann über viele Quellen laufen; ein Suchlauf bleibt als Ergebnis nachvollziehbar.

## Wie Job Radar denkt

Ein typischer Ablauf sieht so aus:

1. **Quellen sammeln**
   Du legst fest, welche Jobquellen relevant sind. Manche Quellen sind eingebaut, andere können lokal ergänzt werden.

2. **Quellen verstehen**
   Wiederverwendbare Quellenprofile beschreiben, wie bestimmte Recruiting-Systeme oder Website-Familien gelesen werden können. Dadurch muss nicht jede Firmen-Karriere-Seite einzeln als Spezialfall behandelt werden.

3. **Suchanfrage formulieren**
   Du definierst, welche Begriffe zählen sollen, welche Begriffe ausgeschlossen werden, welche Orte relevant sind und welche Quellen durchsucht werden.

4. **Suchlauf starten**
   Job Radar holt Kandidaten aus den gewählten Quellen, filtert sie über die Treffer- und Ausschlussregeln, normalisiert die Ergebnisse und führt Dubletten zusammen.

5. **Ergebnisse einordnen**
   Pro Quelle bleibt sichtbar, ob sie erfolgreich war, wie viele Kandidaten sie geliefert hat und ob ein Fehler nur diese Quelle oder den ganzen Suchlauf betrifft.

## Aktueller Stand

Vorhanden sind unter anderem:

- eine lokale Desktop-App auf Tauri-Basis,
- eine Quellen-Übersicht mit eingebauten und lokalen Quellen- und Profil-Dokumenten,
- Diagnosemeldungen für ungültige oder widersprüchliche Quellen-Dokumente,
- eine lokal verwaltete Browser-Laufzeit für browserbasierte Quellen,
- Backend-Logik für Suchanfragen, Suchläufe, Trefferregeln, Ausschlussregeln und Ergebnis-Zusammenführung,
- erste eingebaute Profile für verbreitete Recruiting-Systeme und Jobquellen.

Noch nicht der Anspruch dieser README: alle technischen Details, Schemata und Entwicklungsentscheidungen vollständig zu erklären. Dafür gibt es die tieferen Dokumente unten.

## Wichtige Begriffe

- **Quelle**: ein gespeicherter Ort, aus dem Stellen kommen können.
- **Quellenprofil**: wiederverwendbares Wissen darüber, wie eine Klasse von Quellen erkannt und gelesen wird.
- **Suchanfrage**: die gespeicherte Frage, die du an ausgewählte Quellen stellst.
- **Suchlauf**: eine konkrete Ausführung einer Suchanfrage.
- **Stellenanzeige**: ein normalisiertes Ergebnis, das aus einer oder mehreren Quellen stammen kann.

Das vollständige Projektvokabular steht in [`CONTEXT.md`](CONTEXT.md).

## Orientierung im Repository

Wenn du neu einsteigst, lies am besten in dieser Reihenfolge:

1. diese README — die Produkt- und Einstiegsperspektive.
2. [`CONTEXT.md`](CONTEXT.md) — die gemeinsame Sprache des Projekts.
3. [`docs/prd/declarative-source-profile-dsl.md`](docs/prd/declarative-source-profile-dsl.md) — Zielbild der aktuellen Source Profile DSL.
4. [`docs/adr/0001-source-config-as-json-schema.md`](docs/adr/0001-source-config-as-json-schema.md) und [`docs/adr/0009-declarative-source-profile-dsl.md`](docs/adr/0009-declarative-source-profile-dsl.md) — zentrale Architekturentscheidungen zur Source/Profile DSL.
5. [`docs/adr/`](docs/adr/) — weitere Architekturentscheidungen und ihre Begründungen.
6. [`docs/dev-search-run-smoke.md`](docs/dev-search-run-smoke.md) — manueller Live-Smoke für einen realen Suchlauf.

Die eingebauten Source-Profile liegen in:

- [`src-tauri/resources/profiles/`](src-tauri/resources/profiles/)

## Lokal starten

Für die normale Entwicklung:

```bash
npm install
npm run tauri -- dev
```

Nützliche weitere Befehle:

```bash
npm run build                  # Frontend type-checken und bauen
npm run tauri:dev:reset-db      # Entwicklungsdatenbank zurücksetzen und App starten
npm run smoke:search-run        # manueller, netzwerkabhängiger Suchlauf-Smoke
npm run tauri -- build          # Desktop-App bauen
```

Der Smoke-Test ist bewusst nicht Teil der normalen CI-Logik, weil er echte externe Jobquellen nutzt.

## Nicht-Ziele im Moment

Job Radar ist derzeit kein Bewerbungs-CRM und kein Ersatz für Jobportale. Der Fokus liegt zuerst darauf, die eigene Jobsuche zu automatisieren, Quellen sauber zu beschreiben, Suchläufe nachvollziehbar auszuführen und Ergebnisse verständlich zusammenzuführen.
