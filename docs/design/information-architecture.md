# Informationsarchitektur

Diese Datei beschreibt allgemeine UI-Muster für Job Radar. Sie ist bewusst nicht auf eine einzelne Domäne wie Quellen beschränkt.

## App als Arbeitsoberfläche

Job Radar sollte sich wie eine kompakte Desktop-Arbeitsoberfläche anfühlen, nicht wie eine Marketing-Seite und nicht wie ein reines Backend-Admin-Panel.

Die Oberfläche organisiert wiederkehrende Arbeit über wenige stabile Konzepte:

- **Inbox / Aufmerksamkeit**: Was ist neu, offen oder problematisch?
- **Arbeitsgegenstände**: Dinge, die Nutzer:innen anlegen, konfigurieren oder beobachten.
- **Ausführungen / Läufe**: konkrete Durchführungen zu einem Zeitpunkt.
- **Ergebnisse**: gefundene, erzeugte oder zusammengeführte Resultate.
- **Aktivität**: zeitliche Ereignisse, Statuswechsel, Protokoll in menschlicher Form.
- **Einstellungen**: Präferenzen, Integrationen, erweiterte Konfiguration.

Die konkreten Produktbereiche können sich ändern. Das Muster bleibt: erst Aufmerksamkeit und Arbeit, dann technische Details.

## Standard-Screen-Muster

### 1. Collection Screen

Für vergleichbare Objekte.

Struktur:

```txt
Header: Titel + kurze Erklärung + primäre Aktion
Scope: Tabs oder Segmentierung
Toolbar: Suche, Filter, Sortierung, Ansicht
Main: Data Grid oder Liste
Side: optionales Detailpanel für ausgewählte Zeile
```

Verwenden für Sammlungen wie gespeicherte Arbeitsgegenstände, Ergebnisse, Ausführungen oder Problemlisten.

### 2. Detail Panel

Für Kontext, ohne die Hauptliste zu verlassen.

Geeignet für:

- Zusammenfassung einer ausgewählten Zeile
- Status und nächste Aktion
- relevante Metadaten
- Aktivität/Timeline
- erweiterte technische Details als einklappbarer Abschnitt

Das Detailpanel ist bevorzugt gegenüber einer neuen Seite, wenn Nutzer:innen mehrere Einträge vergleichen oder nacheinander prüfen.

### 3. Timeline / Activity

Für zeitliche oder kausale Informationen.

Verwenden, wenn Reihenfolge wichtig ist:

- was zuletzt passiert ist
- welche Schritte durchgeführt wurden
- wann ein Problem auftrat
- welche Aktion eine Änderung ausgelöst hat

Nicht jede zeitliche Information gehört in eine Tabelle. Logs und Ereignisse sind oft als Timeline verständlicher.

### 4. Modal

Für blockierende, fokussierte Entscheidungen.

Verwenden für:

- Erstellen oder Abschließen einer komplexen Aktion
- Bestätigungen mit Risiko
- mehrstufige Eingaben, die nicht nebenbei passieren sollten

Nicht verwenden für kleine Einstellungen oder sekundäre Aktionen.

### 5. Popover / Dropdown

Für nicht-blockierende Nebenaktionen.

Verwenden für:

- Ansichtsoptionen
- einfache Filter
- Teilen/Exportieren
- kleine Kontextmenüs
- sekundäre Aktionen pro Zeile

### 6. Empty, Loading, Error

Jeder neue Screen braucht bewusst gestaltete Zustände:

- Empty State: Was ist hier möglich? Was ist der erste sinnvolle Schritt?
- Loading State: Welche Struktur wird gerade geladen?
- Error State: Was ist passiert? Was kann die Nutzerin tun?
- Partial Error: Was hat funktioniert und was nicht?

## Navigation

Navigation sollte nach Nutzeraufgaben gruppieren, nicht nach internen Modulen.

Gute Navigationslabels beschreiben Arbeitsbereiche:

- Aufmerksamkeit / Inbox
- Suchen / Arbeitsaufträge
- Ergebnisse
- Aktivität / Läufe
- Sammlungen / Quellen / Integrationen
- Einstellungen

Technische Bereiche gehören in „Erweitert“, „Debug“ oder Einstellungen, nicht prominent in die Standardnavigation.

## Informationsdichte

Job Radar darf dicht sein, weil es eine Desktop-Produktivitätsapp ist. Dichte muss aber geordnet sein:

- klare Tabellen- und Listenstruktur
- Zahlen rechtsbündig
- Status als Chips
- sekundäre Aktionen verstecken
- Details erst bei Auswahl zeigen
- keine unnötigen großen Cards für tabellarische Daten
