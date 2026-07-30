# Job Radar User Guide

Dieser Guide führt durch den ersten vollständigen Ablauf: Job Radar lokal starten, eine **Source** einrichten, eine **Search Request** anlegen und daraus einen **Search Run** ausführen.

> Job Radar ist ein früher Prototyp. Einige Oberflächen verwenden bereits deutsche Beschriftungen, die kanonischen Produktbegriffe bleiben jedoch **Source**, **Search Request** und **Search Run**.

## Was entsteht in diesem Ablauf?

- Eine **Source** speichert, wo und wie Job Radar Stellenanzeigen finden kann. Sie enthält keine Suchbegriffe.
- Eine **Search Request** speichert, wonach gesucht wird: Include- und Exclude-Regeln, optionale Orte und die ausgewählten Sources.
- Ein **Search Run** ist eine konkrete Ausführung dieser Search Request. Er zeigt das Gesamtergebnis und für jede Source einen eigenen **Source Run**.

Eine Source kann später in mehreren Search Requests verwendet werden. Jede erneute Ausführung erzeugt einen neuen Search Run.

## 1. Voraussetzungen und lokaler Start

### Voraussetzungen

Die kanonischen Installationsvoraussetzungen und Startbefehle stehen unter [Lokal starten](../../README.md#lokal-starten) und [Plattform-Builds](../../README.md#plattform-builds) in der Repository-README. Zusätzlich wird für Source Detection, Source Live Checks, Search Runs und gegebenenfalls den Download der Browser Runtime Internetzugriff benötigt.

### Schritte

1. Führe den Abschnitt [Lokal starten](../../README.md#lokal-starten) im Repository-Root vollständig aus.
2. Warte, bis sich die Tauri-Desktop-App geöffnet hat. Die allein mit `npm run dev` gestartete Browseransicht reicht für SQLite- und Tauri-Funktionen nicht aus.

### Erwartetes Ergebnis

Die Desktop-App öffnet sich. Auf der Übersicht meldet der Startcheck **„Alles bereit“** und nennt eine erreichbare SQLite-Version.

### Nächster Schritt

Öffne in der Hauptnavigation **„Quellen“**. Falls der Startcheck fehlschlägt, beginne bei [Startcheck oder App-Start schlägt fehl](#startcheck-oder-app-start-schlägt-fehl).

## 2. Optional: Browser Runtime vorbereiten

Nicht jede Source benötigt einen Browser. Ein browserbasierter Access Path funktioniert jedoch nur mit der von Job Radar verwalteten **Browser Runtime**.

### Voraussetzungen

Die Desktop-App läuft und hat Internetzugriff.

### Schritte

1. Öffne **„Quellen“ → „Browser-Laufzeit“**.
2. Wenn der Zustand **„Nicht installiert“** oder **„Ungültig“** lautet, wähle **„Installieren“**. Bei **„Update erforderlich“** wähle **„Aktualisieren“**.
3. Wähle anschließend **„Prüfen“**.

### Erwartetes Ergebnis

Die Ansicht zeigt **„Installiert“** sowie Version, Installationsordner und Executable. Sources mit reinem HTTP-, API-, Feed- oder Sitemap-Access-Path benötigen diesen Schritt nicht.

### Nächster Schritt

Richte die erste Source ein. Bei einem **„Browser-Laufzeitfehler“** helfen die Hinweise unter [Browser Runtime ist nicht einsatzbereit](#browser-runtime-ist-nicht-einsatzbereit).

## 3. Eine Source einrichten und aktivieren

Für einen Search Run wird mindestens eine valide, aktive und damit ausführbare Source benötigt. Am einfachsten beginnt die Einrichtung mit dem Link zu einer Karriere-Seite oder einem Job-Portal.

### Voraussetzungen

- Die Desktop-App läuft.
- Der Einstiegspunkt der gewünschten Stellenquelle ist als HTTP(S)-Link bekannt.
- Falls der gewählte Access Path browserbasiert ist, ist die Browser Runtime installiert.

### Schritte

1. Öffne **„Quellen“ → „Quellen“** und wähle **„Quelle hinzufügen“**.
2. Trage unter **„Optional: Link prüfen“** den Einstiegspunkt ein und wähle **„Prüfen“**.
   - Bei **„Profil erkannt“** übernimmt Job Radar den Source-Profile-Vorschlag und die erkannten Source-Config-Werte.
   - Bei mehreren Vorschlägen wähle beim passenden Vorschlag **„Übernehmen“**.
   - Wenn die Erkennung keinen ausführbaren Vorschlag liefert, können dieselben Felder manuell ausgefüllt werden. Ohne passendes Source Profile und Access Path ist die Source nicht ausführbar.
3. Prüfe die Felder:
   - **Name** ist die sichtbare Bezeichnung.
   - **Key** ist die stabile Kennung und darf Kleinbuchstaben, Zahlen und Unterstriche enthalten.
   - **Quellenprofil**, **Zugriffspfad** und **Source Config** beschreiben den Zugriff auf die Source. Suchbegriffe gehören ausdrücklich nicht hierher.
4. Wähle **„Quelle speichern“**. Neue Sources werden immer als **„Entwurf“** angelegt; **„JSON ansehen“** zeigt bei Bedarf die authored Definition ohne frei wählbaren Lifecycle-Status.
5. Klicke die neue Zeile in der Quellenliste an, um die Details zu öffnen.
6. Wähle im Abschnitt **„Source Live Check“** die Aktion **„Prüfen & Aktivieren“**.

### Erwartetes Ergebnis

Der Source Live Check ist **„bestanden“**, der Report ist **„frisch“** und der **Source Status** lautet **„Aktiv“**. Eine valide Source wird durch den aktiven Status für normale Search Runs ausführbar; die bestandene Live-Prüfung sichert den Aktivierungsschritt ab.

**„Prüfen“** allein ist statusneutral und aktiviert einen Entwurf nicht. Nach Änderungen an Source Config, Access Path oder Profil kann ein vorhandener Report **„Stale“** werden; führe dann den Check erneut aus. Ein stale Report ändert den Source Status nicht automatisch.

### Nächster Schritt

Öffne **„Search Requests“**. Wenn Speichern, Live Check oder Aktivierung scheitern, lies [Source ist ungültig oder nicht ausführbar](#source-ist-ungültig-oder-nicht-ausführbar) und [Source Live Check schlägt fehl oder ist stale](#source-live-check-schlägt-fehl-oder-ist-stale).

## 4. Eine Search Request erstellen

Die Search Request enthält die Suchabsicht. Mindestens eine Include-Regel und eine Source sind erforderlich, wenn sie direkt aktiv und ausführbar sein soll.

### Voraussetzungen

Mindestens eine Source ist aktiv, valide und ausführbar.

### Schritte

1. Öffne **„Search Requests“** und wähle **„Search Request erstellen“**.
2. Setze den **Status** auf **„Aktiv“**.
3. Lege unter **„Include-Regeln“** mindestens eine Regel an.
   - **Text** sucht derzeit im Titel.
   - **Regex** verwendet die Regex so, wie sie eingegeben wurde; Groß-/Kleinschreibung wird nicht automatisch geändert.
4. Ergänze optional **„Exclude-Regeln“**. Sie entfernen bereits gefundene Treffer; Exclude-Regex-Regeln werden case-insensitive geprüft.
5. Ergänze optional **Orte** (eine Zeile oder kommagetrennt) und einen **Radius (km)**.
6. Wähle unter **„Sources“** mindestens die zuvor aktivierte Source. Ein Badge **„Nicht ausführbar“** weist auf eine noch nicht nutzbare Source hin.
7. Wähle **„Search Request erstellen“**.

### Erwartetes Ergebnis

Die Tabelle enthält eine neue, aktive Search Request ohne Validierungsfehler. Sie zeigt ihre Regeln, Sources, Orte und den Zustand des letzten Laufs.

### Nächster Schritt

Starte die Search Request. Falls **„Search Request noch nicht speicherbar“** erscheint, arbeite die angezeigten Validierungsfehler ab.

## 5. Den ersten Search Run ausführen

### Voraussetzungen

- Die Search Request ist **„Aktiv“** und ohne Validierungsfehler.
- Sie enthält mindestens eine Include-Regel und eine vorhandene, ausführbare Source.
- Die Source sowie gegebenenfalls die Browser Runtime können das Netzwerk erreichen.

### Schritte

1. Öffne in der Zeile der Search Request das Aktionsmenü.
2. Wähle **„Ausführen“**.
3. Beobachte das eingeblendete Panel **„Search Run“**. Der Task wechselt typischerweise von **„Wartet“** zu **„Läuft“** und anschließend zu einem Endzustand.
4. Während der Ausführung kann der Lauf mit **„Abbrechen“** beendet werden.
5. Prüfe nach Abschluss:
   - den Search-Run-Status,
   - **Search-Run-Diagnostics**, falls vorhanden,
   - die **Source-Run Summary** mit dem Ergebnis jeder einzelnen Source.

### Erwartetes Ergebnis

Ein erfolgreicher Lauf trägt den Status **„Abgeschlossen“**. Gefundene und zusammengeführte Stellenanzeigen werden lokal persistiert und sind über **„Stellenanzeigen“** erreichbar. Ein erfolgreicher Lauf darf auch null Treffer enthalten; das ist allein noch kein Fehler.

**„Mit Fehlern abgeschlossen“** bedeutet, dass mindestens eine Source fehlgeschlagen ist. Erfolgreiche Source Runs und ihre Ergebnisse bleiben trotzdem erhalten. **„Fehlgeschlagen“** bezeichnet dagegen einen insgesamt fehlgeschlagenen Search Run.

Die Tabelle zeigt den letzten Lauf der Search Request. Das ausführliche Panel gehört zum gerade gestarteten Lauf; die aktuelle Oberfläche bietet noch keine separate Navigation durch alle älteren Search Runs.

### Sinnvolle nächste Schritte

- Öffne **„Stellenanzeigen“**, um persistierte Treffer zu prüfen.
- Passe Include-, Exclude- oder Ortsregeln über **„Bearbeiten“** an und führe die Search Request erneut aus.
- Ergänze weitere Sources und wähle sie in derselben Search Request aus.
- Nutze bei Teilfehlern die Source-Run-Diagnostics, bevor du den ganzen Lauf wiederholst.

## Troubleshooting

### Startcheck oder App-Start schlägt fehl

- Stelle sicher, dass `npm run tauri -- dev` und nicht nur `npm run dev` läuft. Tauri und SQLite werden nur in der Desktop-App geprüft.
- Lies den ersten Fehler im startenden Terminal. Fehlende Compiler-, WebView- oder Systembibliotheken weisen meist auf unvollständige Tauri-Systemvoraussetzungen hin.
- Führe nach Änderungen an Abhängigkeiten erneut `npm install` aus.
- Prüfe, ob ein anderer Prozess bereits den Vite-Port `1420` verwendet.

### Source Registry konnte nicht geladen werden

- Öffne **„Quellen“ → „Diagnosen“** und beginne bei Diagnostics mit Severity `error`.
- Die Karte **„Eigene Registry-Dateien“** zeigt die tatsächlich verwendeten App-Data-Ordner für `sources/*.json` und `source-profiles/*.json` an.
- Korrigiere oder entferne ein dort abgelegtes ungültiges Custom-Dokument und lade die Registry erneut. Eingebaute Dokumente sollten nicht im App-Data-Ordner dupliziert werden.
- Unter macOS liegt der App-Data-Root derzeit unter `~/Library/Application Support/de.timjonaswechler.jobradar`; auf anderen Plattformen ist der in der App angezeigte Pfad maßgeblich.

### Profilerkennung liefert keinen eindeutigen Vorschlag

- Prüfe, ob der Link direkt zu einer öffentlichen Karriere-Seite oder einem Job-Portal führt und mit `http://` oder `https://` beginnt.
- Bei **„Mehrere passende Profile gefunden“** vergleiche Profil, Access Path und Evidence und wähle **„Übernehmen“**.
- Bei **„Profilerkennung fehlgeschlagen“** oder erreichtem Ausführungslimit prüfe Netzwerkzugriff und Browser Runtime und versuche es erneut.
- Die manuelle Eingabe ist möglich, benötigt aber ein passendes Source Profile, einen Access Path und alle erforderlichen Source-Config-Werte.

### Source ist ungültig oder nicht ausführbar

Eine gespeicherte Source ist nicht automatisch ausführbar. Prüfe in ihren Details:

1. **Validation State** muss valide sein; behebe andernfalls die dort genannten Diagnostics und JSON-Pfade.
2. **Source Status** muss **„Aktiv“** sein. Ein Entwurf wird über **„Prüfen & Aktivieren“** aktiviert.
3. Ein deaktivierter Eintrag wird über **„Prüfen & Reaktivieren“** reaktiviert.
4. Source Profile, Access Path und Source Config müssen weiterhin vorhanden und kompatibel sein.

### Source Live Check schlägt fehl oder ist stale

- Öffne den Source-Detail-Drawer und lies **„Source-Live-Check-Diagnosen“**. Code, Kategorie, Pfad und Strategy Key grenzen den Fehler ein.
- Prüfe URL-/Tenant-/Board-Werte in der Source Config, Netzwerkzugriff und gegebenenfalls die Browser Runtime.
- **„Stale“** bedeutet, dass der letzte Report nicht mehr zu den aktuellen Eingaben passt. Das ist kein neuer Netzwerktest; führe **„Prüfen“** oder **„Prüfen & Aktivieren“** erneut aus.
- Die Aktivierung eines Entwurfs bleibt blockiert, solange die Live-Prüfung nicht besteht. Eine bereits aktive Source wird durch einen später stale gewordenen Report nicht automatisch deaktiviert.

### Browser Runtime ist nicht einsatzbereit

- Öffne **„Quellen“ → „Browser-Laufzeit“** und wähle **„Aktualisieren“**, um den angezeigten Zustand neu zu laden.
- Bei **„Nicht installiert“** oder **„Ungültig“** wähle **„Installieren“**; bei **„Update erforderlich“** wähle **„Aktualisieren“**. Wähle danach **„Prüfen“**.
- Prüfe Internetzugriff, freien Speicherplatz und Schreibrechte für den angezeigten Installationsordner.
- Betrifft die Source keinen browserbasierten Access Path, suche stattdessen in ihren Source-Live-Check- oder Source-Run-Diagnostics.

### Search Request lässt sich nicht speichern oder ausführen

- Eine aktive Search Request braucht mindestens eine nichtleere Include-Regel und eine Source.
- Korrigiere ungültige Regex-Regeln sowie einen Radius, der keine Zahl oder kleiner als null ist.
- Entferne Source Keys mit Badge **„Fehlt“** oder ersetze sie durch vorhandene Sources.
- Öffne Sources mit Badge **„Nicht ausführbar“** und behebe deren Status oder Validation State.
- **„Ausführen“** bleibt außerdem gesperrt, wenn die Search Request nicht aktiv ist, Backend-Validierungsfehler hat oder bereits ein Lauf gestartet wird.

### Search Run endet mit Fehlern oder ohne Treffer

- Öffne zuerst die **Source-Run Summary**. Ein Source Run grenzt einen Fehler auf genau eine Source ein.
- Nutze Diagnostic-Code, Message und Pfad, statt nur den Gesamtstatus zu betrachten.
- **„Mit Fehlern abgeschlossen“** ist ein Teilerfolg: behebe nur die fehlgeschlagenen Sources; erfolgreiche Ergebnisse bleiben erhalten.
- Bei null Treffern prüfe, ob Include-Regeln zu den normalisierten Titeln passen und ob Exclude- oder Ortsregeln zu viel herausfiltern.
- Externe Seiten können vorübergehend nicht erreichbar sein oder ihre Struktur geändert haben. Führe zuerst für die betroffene Source einen neuen Source Live Check aus und wiederhole danach den Search Run.

## Weiterführende Informationen

- [Projektüberblick und Entwicklungsbefehle](../../README.md)
- [Kanonisches Domain-Vokabular](../../CONTEXT.md)
- [Dokumentationsportal](../index.md)
- [Entwicklungsvalidierung](../development/validation.md)
