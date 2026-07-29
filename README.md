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

Das zentrale [`Dokumentationsportal`](docs/index.md) führt nach Zielgruppe und Zweck zu allen gepflegten Dokumentationsbereichen.

- **Job Radar verwenden:** Der [`User Guide für den ersten vollständigen Search Run`](docs/user-guide/README.md) führt vom lokalen Start über Source und Search Request bis zu Ergebnissen und Troubleshooting; diese README bleibt Produktüberblick und kurzer lokaler Einstieg.
- **Job Radar entwickeln:** mit [`Development`](docs/development/README.md) und dem [`CONTEXT.md`](CONTEXT.md)-Vokabular beginnen.
- **Entscheidungen und Anforderungen verstehen:** die Einstiege für [`Architecture`](docs/architecture/README.md) und [`Product`](docs/product/README.md) verwenden.
- **Fakten nachschlagen oder interne Arbeit einordnen:** zu [`Reference`](docs/reference/README.md) beziehungsweise [`Project-internal work`](docs/project/README.md) wechseln.

Bestehende ADR-, PRD- und Entwicklungslinks bleiben während der schrittweisen Migration gültig. Die eingebauten Source-Profile liegen in:

- [`src-tauri/resources/profiles/`](src-tauri/resources/profiles/)

## Lokal starten

Vorausgesetzt werden Node.js mit `npm`, eine Rust-Toolchain und die [Tauri-2-Systemvoraussetzungen](https://v2.tauri.app/start/prerequisites/) für das eigene Betriebssystem. Für die normale Entwicklung im lokalen Repository-Checkout:

```bash
npm install
npm run tauri -- dev
```

Nützliche weitere Befehle:

```bash
just quick                     # schnelle Typechecks, Frontendtests und cargo check --tests
just frontend-test settings    # Frontendtest über Vitest filtern
just rust-test profile_dsl_profiles workday:: # ein sichtbares Rust-Testtarget fokussieren
just verify                    # vollständiges lokales Übergabe-Gate
just package                   # Desktop-App für das aktuelle System bauen
npm run test:frontend:watch -- settings       # Vitest im gefilterten Watch-Modus
npm run smoke:search-run       # manueller, netzwerkabhängiger Suchlauf-Smoke
```

Die drei Ebenen sind bewusst getrennt: `quick` bündelt den schnellen hermetischen Loop ohne Produktionsbundle und ohne vollständigen Cargo-Testlauf; fokussierte Rezepte führen ein gewähltes Testtarget aus; `verify` ist vor jeder Übergabe erforderlich und wird weder durch Quick- noch durch Focused-Läufe ersetzt. Die vollständige Befehlsreferenz einschließlich Plattformhinweisen steht in [`docs/development/validation.md`](docs/development/validation.md). Credential-Scanner und Scanner-Self-Test bleiben eigenständige Security-Kommandos.

Der Smoke-Test ist bewusst nicht Teil der normalen CI-Logik, weil er echte externe Jobquellen nutzt.

## Plattform-Builds

Die Desktop-Bundles werden pro Zielsystem gebaut. Die CI führt `npm run tauri -- build` deshalb auf macOS, Windows und Linux aus. Für lokale Linux-Builds müssen vorher die nativen Tauri-Abhängigkeiten installiert sein, zum Beispiel unter Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential curl wget file \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libssl-dev libxdo-dev patchelf rpm
```

## Nicht-Ziele im Moment

Job Radar ist derzeit kein Bewerbungs-CRM und kein Ersatz für Jobportale. Der Fokus liegt zuerst darauf, die eigene Jobsuche zu automatisieren, Quellen sauber zu beschreiben, Suchläufe nachvollziehbar auszuführen und Ergebnisse verständlich zusammenzuführen.
