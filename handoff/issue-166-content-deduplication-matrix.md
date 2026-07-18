# Issue #166 — Content-Deduplication-Matrix

Status: **lokale Review-Artefakt — noch keine GitHub-Issues ändern**  
Stand: 2026-07-17

## Zweck

Diese Matrix bereitet ausschließlich die redaktionelle Normalisierung der zu #166 gehörenden 27 Implementierungs-Issues vor. Sie ändert noch keine Ticketgrenzen, Abhängigkeiten oder fachlichen Entscheidungen.

Review-Basis:

- Parent #166;
- vollständige Bodies von #167–#180, #192, #193, #195, #202–#207, #218, #219, #233 und #234;
- `CONTEXT.md`;
- `docs/prd/declarative-profile-strategy-algebra.md`;
- ADRs 0001, 0008, 0009 und 0010;
- `docs/dev-search-run-smoke.md`;
- `handoff/issue-166-ticket-template.md` und `handoff/issue-166-ticket-index.md`;
- native GitHub-Parent- und Dependency-Beziehungen.

Die 27 Ticketbodies umfassen derzeit ungefähr 175.000 Wörter. Ziel der Bereinigung ist nicht, Verträge zu entfernen, sondern jeden Vertrag genau einmal kanonisch zu halten und in Tickets nur das jeweilige Delta zu belassen.

## Vorgeschlagene Quellenhierarchie

| Inhalt | Kanonischer Ort | Nicht kanonisch |
|---|---|---|
| Domain-Begriffe und Abgrenzungen | `CONTEXT.md` | Implementierungsdetails, Schemaskizzen, Delivery-Regeln |
| Produkt- und übergreifender Architekturvertrag der Serie | `docs/prd/declarative-profile-strategy-algebra.md` / #166 | Kopien in jedem Child Ticket |
| Schwer reversible Architekturentscheidungen | passende ADRs | Ticket-Historie und temporäre Zwischenzustände |
| Repositoryweite Arbeits- und Testregeln | `AGENTS.md`, `docs/agents/*` | wiederholte Lehrtexte in Tickets |
| Serienbezogene Delivery-, Migration- und PR-Evidence-Regeln | vorgeschlagenes `handoff/issue-166-delivery.md` | vollständige Checklisten in jedem Ticket |
| Aktuelle Parent-, Dependency-, Label- und Readiness-Daten | natives GitHub; lokaler Index nur als Spiegel | datierte Tracker-Snapshots in Ticketbodies |
| Konkretes Implementierungsdelta | jeweiliges Ticket | PRD-/ADR-Wiederholungen |

## A. Architektur- und Domain-Deduplizierung

| Cluster | Betroffene Issues | Kanonischer Zielort | Aus Tickets entfernen/verkürzen | Im Ticket verbleibendes Delta | Abweichungen, die nicht blind zusammengeführt werden dürfen |
|---|---|---|---|---|---|
| Schema-v3-Vokabular und Hard Cut | praktisch alle; Owner #173/#174 | PRD Decisions 1, 36–38; aktualisierte ADR 0009 | wiederholte Verbote von v2-Runtime, Alias, Wrapper und Parallelmodell | konkret umbenannte/gelöschte Symbole, Fixtures und Caller | #173 beschreibt bewusst den internen Zwischenschritt; #174 den authored Hard Cut |
| Effective Profile Compiler | #167–#171, konsumiert von späteren Compiler-/Runtime-Tickets | PRD „Module and interface decisions“; ggf. kompakter Compiler-Abschnitt | autoritative Source, Snapshot-ist-Daten, Lifecycle außerhalb, immutable Plan nicht überall wiederholen | vom Ticket neu eingeführte Eingabe/Ausgabe und Compilerstufe | keine fachliche Abweichung; spätere Typnamen dürfen dem gelandeten Code folgen |
| Direct Source Specialization und Merge | #167–#171, #174–#175, #177–#178, #192–#195, #202–#204 | PRD Decisions 12–21, 38, 47; aktualisierte ADRs 0001/0009 | Wrapper-/Patch-Verbote, stable-key-, Array-, `null`- und Reihenfolgeregeln | exakt neu zugelassene Felder, Completeness und Diagnostics | `required: []` ist Whole-array-Replacement, keine strukturelle Löschung |
| Source Config vs. Search Request | breite Serie, besonders #170, #193, #233, #234 | `CONTEXT.md`, PRD Decision 11/47, ADR 0001 | vollständige globale Verbotsliste aus jedem Non-Goals-Abschnitt | ticketspezifische Grenze: Schema, Discovery Values, Matching oder Persistenz | ADR 0001 ist bei Schema-Spezialisierung veraltet, bei Search-Request-Trennung weiterhin richtig |
| Strategy-Set-Algebra und Runtime-Grenze | #172, #176–#177, #195, #202–#207 | PRD Decisions 2–10 und Runtime-Modulentscheidung | kein generischer Public Executor/Plugin/Policy-Trait in jedem Ticket wiederholen | Policy-Transition, Phase-Adapter oder Ledger-Delta | `first_accepted`, `all_required`, `at_least`, `collect_all` behalten unterschiedliche Stop-Regeln |
| Acceptance und konfliktfreie Reducers | #172, #176–#177, #195, #202–#206 | gemeinsame Policy-Matrix im PRD; Reducer-Vertrag bei #195/#206 | „Transport success != accepted“ und no-last-write-wins nur einmal kanonisch | genaue Konfliktverantwortung, Reducer-Input und Terminaldiagnostic | Posting- und Detection-Reducer verwenden getrennte Domain-Typen |
| Boundedness, Limit Ownership und Cancellation | #169, #176–#180, #192–#195, #202–#207, #218–#219, #233–#234 | PRD Decisions 27–28, 41–45, 48 plus gemeinsame Limit-Tabelle | generische Regeln zu Tightening, Debit-before-side-effect, Cancellation und Statusvarianten | neue Dimension, exakte Ceiling, Precedence und beobachtbares Ergebnis | Phase Budgets, Browser Ceilings und Candidate-Resolution-Budgets bleiben unterschiedliche Ebenen |
| Provider-neutrale Primitives und Single Ownership | gesamte Serie; Implementierungsowner #178–#180/#192 | PRD Decisions 23, 29–30, 39–40; aktualisierte ADR 0009 | globale „keine ATS-/Host-/Key-Branches“-Listen | konkrete Primitive-Familie, Registry-Einträge und Context-Checks | Registry Completeness ist bis #192 absichtlich familienweise, nicht global |
| Detection, Source Proposal und Operational Confidence | #170, #171, #175, #205–#207, #218 | PRD Decisions 8, 14, 33; ADR 0010 | Profile-Support-vs.-Live-Check und allgemeine Detection-Grenzen | URL/HTTP-, Reducer-, Browser- oder Fingerprint-Delta | #205/#206 enthalten derzeit absichtliche Übergangszustände; nicht als finalen Vertrag kanonisieren |
| PostingOccurrence, Provider Values und Hints | #193, #195, #204, #219, #233–#234 | PRD Decisions 24, 31–32, 46; später `CONTEXT.md` | Identität und Hint-Trust-Regeln nicht in jedem Downstream-Ticket wiederholen | neue Shape-, Reducer-, Request- oder Finalization-Regel | #193 bewahrt Provider-URL vor Reduction; #195 normalisiert URL-Fallback im reduzierten Public Result |
| Requested Detail und Lazy Enrichment | #193, #195, #202–#204, #219, #233 | PRD Decisions 10, 41–42; später `CONTEXT.md` | Lazy-/requested-only-/multi-field-Grundvertrag | Patch-/Reducer-, Capability-/Disposition- oder Resolution-Delta | „required fields“ der Candidate Resolution und „requested fields“ des Detail Calls nicht vermischen |
| Candidate Resolution, Finalization und Persistenz | #193, #195, #219, #233, #234 | PRD Decisions 41–46; Candidate-Resolution-Dokument; neue/supersedende Persistenz-ADR | Count-, Cancellation-, Finalized-only- und Dedup-Grenzen nicht mehrfach kopieren | #233: Zustände/Counts/Diagnostics; #234: Merge/DB/Transaction | ADR 0008 ist bis zur Ablösung aktuelle Implementierungsdoku, aber nicht mehr Zielentscheidung |
| Structured Diagnostics, Provenance und Data Minimization | #168–#180, #192–#219, #233–#234 | PRD-Taxonomie für Diagnostic-/Provenance-Arten | wiederholte Secret-Denylists und generische Diagnostic-Felder | neuer Code, Pfad, Reihenfolge, Origin-Typ und Volumenlimit | Compiler-, Runtime-Attempt-, Detection- und Contribution-Provenance dürfen nicht vereinheitlicht werden |
| Live-Check-Freshness | #171, #175; spätere Behavior-Version-Owner | ADR 0010 plus PRD Decision 33 und #175-Komponententabelle | vollständige Fingerprint-Komposition aus späteren Runtime-Tickets | welche Behavior-Version das Ticket ändern muss | Support Level bleibt Domain-Metadatum; Freshness bleibt konkreter Source Check |

## B. Delivery-, Test- und Tracker-Deduplizierung

Alle 27 Tickets besitzen derzeit dieselben 17 Hauptabschnitte. Das ist durch das bestehende Template erzwungen und Hauptquelle der Textmenge.

| Cluster | Kanonischer Zielort | Aus Ticketbodies entfernen | Ticketspezifisch behalten |
|---|---|---|---|
| Parent, Dependencies, Readiness | Live GitHub; `docs/agents/issue-tracker.md`; Index als Spiegel | Publikationshistorie, Zeitstempel, Placeholder-Regeln, wiederholte Label-Policy | Parent, direkte Blocker, aktueller Readiness-Satz, echte offene Entscheidung |
| Shared #166 invariants | PRD + gemeinsames Delivery-Dokument | Cancellation/Counts/Status/Source-authority/typed-plan-Grundtexte | konkrete Auswirkung auf die neue API und Acceptance Cases |
| Current State | beim Readiness-Review neu erhobener Ticketabschnitt | Commit-Hashes, Dirty-Tree-Snapshots, pre-T1-Erzählung, hypothetische zukünftige Typnamen | existierende Pfade/Interfaces/Tests und die konkrete Lücke zum Zeitpunkt der Freigabe |
| Restatement von Blockern | PRD + gelandeter direkter Blocker | transitive Vertragskopien und komplette Vorgängerspezifikationen | direkte Blocker und 2–5 tatsächlich konsumierte Annahmen |
| Dependency Categories/Seams | Architecture-Language-Regeln + Delivery-Dokument | Definitionen aller Kategorien und Standardverbote | nur Klassifizierungen, die eine Design-/Testentscheidung des Tickets verändern |
| Deletion Test | Delivery-Dokument erklärt Methode | generische Erklärung des Tests | ein ticketspezifischer Satz: welche Komplexität würde in welche Caller zurücklaufen? |
| Test-Seam-Regeln | `AGENTS.md` + Delivery-Dokument | Standardtext zu externen Tests, Fakes und network-free CI | Interface, repräsentierter Caller, realer/Fake-Teil und Assertion für dieses Verhalten |
| Test Commands | `AGENTS.md` für Baseline | vollständige, überall gleiche Rust-/Acceptance-Profile-Suite | fokussierte Targets, besondere Regressionen und ticketspezifische `rg`-Checks |
| Migration/Deletion Checklist | gemeinsames Delivery-Dokument | generische Add/Migrate/Delete/Test/Search-Checkliste | konkrete alte Symbole/Pfade/Caller und erwartete Suchergebnisse |
| Non-Goals | PRD-Deferrals + `AGENTS.md` | globale Verbotslisten | nur angrenzende Arbeit, die hier realistisch versehentlich hineingezogen würde |
| Definition of Done | gemeinsames Delivery Gate | Wiederholung von Acceptance, Tests, Tracker, Branch-, Port- und CI-Verboten | seltene outcome-spezifische Bedingung; sonst Verweis auf Matrix + Shared Gate |
| Required PR Attestation | vorgeschlagenes PR-Template oder gemeinsamer Evidence-Abschnitt | kompletten Abschnitt aus allen 27 Tickets entfernen | Ticket-Acceptance definiert Belege; PR berichtet Pfade, Löschungen, Commands und Restrisiko |

## C. Materielle Konflikte und veraltete Aussagen

Diese Inhalte dürfen beim Kürzen nicht als harmlose Dubletten behandelt werden.

| Konflikt | Klassifikation | Kanonischer Owner / spätere Aktion |
|---|---|---|
| `Source Overrides` in `CONTEXT.md`/ADRs vs. Direct Source Specialization | durch #166 entschieden | #174 aktualisiert/superseded ADR 0001/0009 und Glossar |
| Aktiver alter `sourceOverrides`-Pfad zwischen T1 Direct Fragments und dem T7 Hard Cut | Delivery-Route-Konflikt; nicht durch Lean-Kürzung lösen | Bei fachlicher Neustrukturierung explizit festlegen: im selben Slice entfernen/invalidieren oder einen sicheren nicht-ausführbaren Zwischenvertrag definieren; keine zwei aktiven Spezialisierungsmodelle |
| T14b übersetzt heutigen aggregierten Browser-Output erst nach möglichem Capture-Overwrite und benötigt zugleich pre-browser Source Config für Browser-Templates | Delivery-Route-/Zwischenzustandskonflikt aus dem veröffentlichten Vertrag | Bei Detection-Neustrukturierung festlegen, wie jede geordnete Browser-Probe verlustfrei vor Overwrite in Contributions überführt wird und wie reconciled pre-browser Source Config ohne zweiten Merge-/Proposal-Pfad bereitsteht |
| T14c ersetzt den gemeinsam von Detection, Discovery, Detail, Source Live Check und Search Run genutzten `ProfileBrowserClient`-Seam, erklärt diese Caller aber teilweise zu Non-Goals | Scope-/Dependency-Konflikt aus dem veröffentlichten Vertrag | T14c entweder auf einen Detection-spezifischen Lifecycle-Adapter begrenzen oder die vollständige Migration und Paritätscoverage aller gemeinsamen Browser-Caller übernehmen |
| T14c/T14d verlangen „recovered fallback“, obwohl Detection in T14a–T14b ausschließlich `all_required` fail-fast verwendet | Acceptance-/Policy-Konflikt aus dem veröffentlichten Vertrag | Beim Route-Redesign entfernen oder den exakt gemeinten inneren Fallback samt Owner benennen; keine neue Detection Policy implizit einführen |
| T15 klassifiziert budget-terminal fehlende Felder als `Unavailable`, während T12b bei Budget Exhaustion keinen reduzierten Patch-/Konflikt-/Provenance-Payload freigibt | Result-Algebra-Konflikt zwischen veröffentlichten/normalisierten T12b- und T15-Verträgen | Vor T15-Freigabe festlegen, wie der unteilbare `StrategySetBudgetReport` ohne erfundene Field Dispositions den Source-Detail-Seam durchquert; Budget Exhaustion nicht still in gewöhnliches `Unavailable` umdeuten |
| T13a/T13b/T13c sind tracker-seitig unabhängige Geschwister, ändern aber dieselben geschlossenen Policy-/Result-/Kernel-Flächen | Delivery-/Merge-Risiko | Lean-Tickets besitzen eine First-Lander-Regel für die gemeinsame Result-Algebra; bei späterer Dependency-Neustrukturierung serielle Landung oder einen expliziten Foundation-Owner bevorzugen, keine parallelen Implementierungs-Worker |
| `postingDiscovery`/`postingDetail`, vollständige Discovery-Werte und description-only Detail | Ziel entschieden, Implementierungsdoku noch alt | #174 Phasennamen; #193 Occurrence/Hint-Vertrag; #219 Multi-field Detail |
| SCHOTT-Smoke erwartet URL-abgeleitete kanonische Titel/Location | Implementierungszeitpunkt | #193 entfernt Hint→Canonical; #233 aktualisiert finalen Pipeline-Smoke |
| Smoke beschreibt unbounded Raw Candidates/alten Artifact-Flow | Implementierungszeitpunkt | #174 benennt um; #233 entscheidet bounded/sanitized Artifact-Verhalten |
| ADR 0008 lehnt Search-Run-Historie ab, #234 verlangt `search_runs`/`matches` | explizit durch #234-Approval revidiert | #234 aktualisiert oder superseded ADR 0008 im selben Slice |
| Provider-URL-Repräsentation #193 vs. reduzierter URL-Fallback #195 | beabsichtigte spätere Verfeinerung | #193 besitzt Pre-reduction Shape; #195 den reduzierten Public Result |
| PRD Decision 49 behauptet offenen T16-Sample-Limit-Gate | historisch veraltet | PRD später auf „Gate erfüllt, Limit 10“ aktualisieren; #233 ist numerischer Owner |
| Child-Tickets behaupten, Dependents seien noch nicht publiziert | historisch veraltet | entfernen; Live GitHub ist kanonisch |
| #167 und mehrere frühe Tickets enthalten alte Publication-/Readiness-Texte | historisch veraltet | beim Ticket-Cleanup durch einen aktuellen Readiness-Satz ersetzen |
| #206 nennt eine „recommended“ Schema-Erweiterung trotz festem Target Contract | missverständliches Modalwort | bei Rewrite als „selected/required“ formulieren |

### Bereits sicher veraltete Tracker-Prosa

Mindestens #167, #173, #174, #176, #177, #179, #180, #192, #193, #195, #205, #207, #219 und #233 enthalten Publikations- oder Placeholder-Aussagen, die dem heutigen veröffentlichten Graphen widersprechen. Diese Sätze beschreiben Autorengeschichte und sollten ersatzlos entfallen, nicht fortlaufend aktualisiert werden.

## D. Ticket-spezifischer Restinhalt nach der Deduplizierung

Diese Tabelle beschreibt noch **keine** Merge-/Split-Entscheidung. Sie zeigt nur, was nach Entfernung gemeinsamer Inhalte fachlich im jeweiligen aktuellen Ticket verbleiben muss.

| Issue | Verbleibender Kern |
|---|---|
| #167 | autoritative Compiler-Schnittstelle, ein minimaler Direct-Fragment-Beweis, Caller-Migration und Löschung des alten Entry Points |
| #168 | rekursiver Merge bestehender keyed Entries, Whole-array-Replacement, Order und Diagnostics |
| #169 | vollständige neue Strategies/Access Paths, Append Order, Completeness und Auswahl neuer Paths |
| #170 | constrained Schema-Sprache, Komposition/Spezialisierung, gemeinsamer Validator und Diagnostic Mapping |
| #171 | konkrete Provenance-Shape, Origin-Regeln, Coverage und inspectable Compiler Result |
| #172 | compiled `first_accepted`, exakte Fallback-/Recovery-/Cancellation-Parität |
| #173 | ausschließlich interne Phase-Umbenennung und konkrete Löschsuche |
| #174 | authored schema-v3-Hard-Cut, Policy-Pflicht, Ressourcen-/Docs-Migration und v2-Löschung |
| #175 | konkrete Fingerprint-Komponenten, Version Ownership, Fresh/Stale-Übergänge und Data Minimization |
| #176 | privater Strategy-Set-Kernel, typed attempts/terminals, Phase Adapter und Caller-Migration |
| #177 | Ledger-Dimensionen, Limit-Precedence, Debit-Stellen, Usage/Completion und Boundary Tests |
| #178 | Byte-Response-/Decoder-Vertrag, Streaming/Charset/Sanitization und Byte-Accounting |
| #179 | Parse-Primitive-Owner, Parsed Document, Registry/Completeness und Parse Diagnostics |
| #180 | Select-Primitive-Owner, Context/Syntax Validation und Phase-spezifische Cardinality |
| #192 | Value-Primitive-Owner, typed contexts, `first_non_empty`, Expression Bounds und Capture-Hard-Cut |
| #193 | `PostingOccurrence`, Source-lokale Identity, Provider Values/Hints und URL-Sicherheitsregeln |
| #195 | requested Detail Patch, Contribution Provenance und konfliktfreie Phase Reducers |
| #202 | `all_required`-Transitions, Fail-fast, Reducer-Aufruf und Policy Diagnostics |
| #203 | `at_least(count)`-Cardinality, earliest success/impossibility und Diagnostics |
| #204 | `collect_all(minAccepted)`, execute-all und Discovery Union über bestehenden Reducer |
| #205 | authored URL/HTTP Detection, Detection Context, all-required-Ausführung und alte HTTP-Pfad-Löschung |
| #206 | Detection Contributions, Konfliktreducer, Source-Proposal-Provenance und sole constructor |
| #207 | Browser Strategy, Lifecycle/Teardown, mehrstufige Ceilings und alte Browser-Pfad-Löschung |
| #218 | post-migration Call-Graph-Prüfung und optionaler Convergence Guard; keine neue Runtime-Fähigkeit |
| #219 | Source Detail Request, Capability/Reuse-Routing, Field Dispositions und UI-/Live-Check-Migration |
| #233 | Batch Protocol, Candidate State Machine, Detail Rounds, Counts/Completion, Diagnostic Sampling und Search-Run-Integration |
| #234 | Finalized-only Handoff, cross-Source Merge, dauerhafte Search Runs/Matches und atomare Persistenz |

## E. Vorgeschlagene gemeinsame Dokumente

### 1. Bestehendes PRD als Architekturquelle schärfen

Kein zweites vollständiges Architektur-Dokument anlegen. Das vorhandene PRD sollte später:

- eine kompakte Compiler-/Merge-Sektion;
- eine Policy-Matrix;
- eine Limit-Ownership-Tabelle;
- eine Diagnostic-/Provenance-Taxonomie;
- den erfüllten T16-Sample-Limit-Gate mit Wert 10

enthalten.

### 2. `handoff/issue-166-delivery.md` neu anlegen

Inhalt:

- Readiness-/Re-baseline-Regel;
- Hard-Cut- und Löschpflicht;
- allgemeiner Deletion Test;
- Test-Seam- und deterministic-adapter-Regeln;
- Standard-Regressionen;
- gemeinsame Migration Checklist;
- gemeinsames PR-Evidence-Gate.

### 3. Tickettemplate radikal verkürzen

Das Template soll nur noch ticketindividuelle Felder verlangen und auf PRD/Delivery Gate verweisen. Es darf nicht erneut deren Inhalte kopieren.

### 4. ADR-/Glossar-Aktualisierungen an ihren Implementierungsschnitt binden

- #174: ADR 0001/0009 und Direct-Specialization-/Phase-Vokabular;
- #193/#219: PostingOccurrence, Hints, Provider Values und Multi-field Detail in `CONTEXT.md`;
- #234: ADR 0008 superseden;
- #174/#193/#233: Smoke-Dokument schrittweise an die tatsächlich gelandete Pipeline anpassen.

## F. Lean Ticketformat für Phase 2

```md
# <Outcome-orientierter Titel>

## Ergebnis
<ein beobachtbares Ergebnis>

## Readiness und direkte Blocker
- Parent #166
- direkte Blocker
- aktueller Readiness-Satz / echte offene Entscheidung

## Konsumierte Verträge
- Links auf konkrete PRD-/ADR-/Delivery-Abschnitte
- nur 2–5 Annahmen des direkten Blockers

## Aktuelle Lücke
- existierende Pfade/Interfaces/Tests nach Readiness-Rebaseline

## Contract-Delta
- neue/geänderte Inputs und Outputs
- ticketspezifische Invarianten, Ordering, Fehler und Diagnostics
- was Caller danach nicht mehr wissen

## Scope und angrenzende Non-Goals
- Implementierung, Caller-Migration und Löschung
- nur realistisch verwechselbare Nachbararbeit

## Akzeptanz und Validierung
- kompakte Matrix: Fall → Ergebnis → Test/static check
- fokussierte Commands
- ticketspezifische Löschsuchen

## Delivery Gate
Folgt `handoff/issue-166-delivery.md`.
```

## G. Erwartete Reduktion

Ein realistisches Ziel nach Phase 2 ist:

- Standardtickets: etwa 1.500–2.000 Wörter;
- große Contract-/Migrationstickets: etwa 2.500–3.500 Wörter;
- gemeinsame Architektur-, Delivery- und PR-Regeln jeweils nur einmal.

Damit sollte die Serie von ungefähr 175.000 auf etwa 45.000–70.000 Wörter sinken, ohne ticketspezifische Interfaces, Akzeptanzfälle oder Löschbelege zu verlieren.

## Nächster Review-Gate

Vor jeder GitHub-Änderung sollten gemeinsam freigegeben werden:

1. die Quellenhierarchie;
2. die Architektur-/Domain-Cluster A;
3. der Umgang mit den materiellen Konflikten C;
4. das Lean Ticketformat;
5. ob `handoff/issue-166-delivery.md` und ein PR-Template tatsächlich angelegt werden sollen.

Erst danach sollte Phase 2 einen lokalen gekürzten Ticketentwurf erzeugen. Ticketgrenzen und Dependency-Graph bleiben bis zur anschließenden fachlichen Neustrukturierung unverändert.
