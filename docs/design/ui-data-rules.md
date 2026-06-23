# Regeln für Daten in der UI

## Grundsatz

Backend-Daten sind Inputs für die UI, aber keine automatische UI-Spezifikation.

Agenten und Entwickler:innen dürfen nicht alle verfügbaren Felder anzeigen, nur weil sie existieren. Jede sichtbare Information braucht einen Zweck.

Gleichzeitig muss jede sichtbare Information fachlich korrekt aus Backend- und Domainzuständen ableitbar sein. Für Status- und Badge-Mapping siehe [`domain-ui-contract.md`](domain-ui-contract.md).

## Sichtbarkeitsprüfung

Bevor ein Feld in die UI kommt, eine der Fragen mit „ja“ beantworten:

1. Hilft es bei einer unmittelbaren Nutzerentscheidung?
2. Erhöht es Vertrauen in ein automatisches Ergebnis?
3. Erklärt es ein Problem in verständlicher Sprache?
4. Zeigt es eine sinnvolle nächste Aktion?
5. Ist es für Vergleich, Priorisierung oder Wiederfinden nötig?

Wenn keine Frage zutrifft, bleibt das Feld verborgen oder kommt in eine erweiterte/debug-nahe Ansicht.

## Progressive Offenlegung

Informationen werden in Ebenen gezeigt:

1. **Übersicht** — Status, Name, wichtigste Zahl, letzte Aktivität, nächste Aktion.
2. **Zeile/Karte** — kurze Vergleichsinformationen, kompakte Badges, wenige relevante Spalten.
3. **Detailpanel** — Kontext, Zusammenfassung, Timeline, Beziehungen, handlungsnahe Details.
4. **Erweitert** — technische Ursache, Konfiguration, selten genutzte Optionen.
5. **Debug/Rohdaten** — interne Keys, IDs, JSON, Adapter-/Runtime-/Schema-Details, Logs.

Standardansichten dürfen höchstens Ebene 1–3 dominieren. Ebene 4–5 ist bewusst verborgen.

## Welche Daten meistens sichtbar sind

- menschlicher Name oder Titel
- verständlicher Status
- kurze Beschreibung oder Zweck
- letzte relevante Aktivität
- nächste empfohlene Aktion
- relevante Anzahl oder Änderung
- betroffene Objekte in menschlicher Sprache
- Warnung/Problem mit verständlicher Ursache

## Welche Daten meistens verborgen bleiben

- interne IDs und technische Keys
- rohe JSON-Dokumente
- Schema-Details
- Adapter-, Runtime- oder Implementierungsnamen
- vollständige Logs
- technische Fehlercodes ohne Übersetzung
- Backend-Zwischenzustände ohne Nutzerwirkung

Diese Daten können im Debug-/Maintainer-Modus existieren, sollen aber nicht die Hauptoberfläche prägen.

## Darstellung nach Datentyp

### Status

Status wird als Chip/Badge dargestellt, nicht als lange Textspalte oder große farbige Fläche.

Statusfarben sind semantisch:

- Erfolg/aktiv: grün, weich
- Warnung/offen: gelb/amber, weich
- Fehler/invalid: rot, weich
- Info/laufend: blau/violett, weich
- Entwurf/disabled: neutral/muted

### Zahlen

- in Tabellen rechtsbündig
- mit Einheit oder Kontext
- nicht ohne Aussagekraft als große Metrik anzeigen
- Deltas nur zeigen, wenn Vergleich relevant ist

### Zeit

- relative Zeit für schnelle Orientierung: „vor 5 Minuten“, „heute 09:30“
- absolute Zeit im Detail oder Tooltip
- chronologische Vorgänge als Timeline statt als rohe Tabelle

### Aktionen

- eine primäre Aktion pro Bereich
- sekundäre Aktionen in Menüs, Popovers oder Hover-Bereichen
- gefährliche Aktionen nicht prominent, sondern bestätigt und kontextualisiert

### Fehler

Fehler müssen menschlich sein:

```txt
Nicht: missing_path_ref
Sondern: Der ausgewählte Zugriffspfad existiert nicht mehr.
Aktion: Konfiguration prüfen
Details: Technischen Fehler anzeigen
```

## Agentenregel

Wenn Agenten eine UI aus Backend-Daten bauen, müssen sie zuerst eine kleine Sichtbarkeitsmatrix erstellen:

```txt
Feld | Für Nutzer sichtbar? | Warum? | Ebene
```

Felder ohne klaren Zweck werden nicht in der Standardansicht angezeigt.
