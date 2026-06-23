# Produktperspektive und Zielgruppe

## Primäre Zielgruppe

Job Radar wird primär für Menschen gestaltet, die wiederkehrende digitale Arbeit beobachten, ausführen und einordnen müssen — im aktuellen Produktkontext: die eigene Jobsuche strukturierter, wiederholbarer und nachvollziehbarer machen.

Die primäre Nutzerin ist nicht in erster Linie Entwickler:in, Datenmodell-Expert:in oder Maintainer:in. Sie will eine Arbeitsoberfläche, die Orientierung gibt:

- Was ist neu?
- Was läuft gerade?
- Was braucht meine Aufmerksamkeit?
- Was ist abgeschlossen?
- Was ist fehlgeschlagen und was kann ich dagegen tun?
- Kann ich dem Ergebnis vertrauen?

## Sekundäre Zielgruppen

Sekundäre Zielgruppen dürfen unterstützt werden, sollen aber die Hauptoberfläche nicht dominieren:

- Power User, die Details prüfen möchten.
- Maintainer, die Konfiguration und technische Diagnose verstehen müssen.
- Entwickler:innen und Agenten, die Implementierungs- oder Debug-Informationen benötigen.

Diese Informationen gehören in progressive Offenlegung: Detailpanel, erweiterte Abschnitte, Debug-Modus oder Rohdatenansicht.

## Nicht-Ziel der Haupt-UI

Die Haupt-UI ist nicht:

- ein 1:1-Spiegel des Backend-Datenmodells,
- eine Admin-Konsole für jede interne Entität,
- eine rohe Log-/JSON-Ansicht,
- ein technisches Diagnose-Dashboard als Standardansicht,
- eine Sammlung bunter Metrik-Karten ohne konkrete Nutzerentscheidung.

## Produktversprechen der UI

Die UI soll Nutzer:innen das Gefühl geben:

> „Ich sehe auf einen Blick, wo meine Aufmerksamkeit gebraucht wird, was zuletzt passiert ist und ob ich dem Ergebnis vertrauen kann.“

## Nutzermodi

### Normaler Arbeitsmodus

Der Standard. Zeigt verständliche, handlungsnahe Informationen:

- Name, Beschreibung und Status eines Arbeitsgegenstands
- letzte Aktivität und nächste sinnvolle Aktion
- Ergebniszusammenfassung
- Warnungen und Fehler in Alltagssprache
- relevante Zählwerte, Deltas und Zeitpunkte

### Erweiterte Ansicht

Für Nutzer:innen, die mehr Kontrolle brauchen:

- technische Ursache eines Problems
- verwendete Konfiguration in lesbarer Form
- detaillierte Ausführungsschritte
- erweiterte Filter und Sortierung

### Debug-/Maintainer-Modus

Nicht Standard. Für Entwicklung, Fehlersuche und Agentenarbeit:

- interne Keys und IDs
- Adapter-/Runtime-/Profilinformationen
- rohe Diagnosedaten
- JSON- oder Schema-Ansichten
- technische Logs

## Design-Implikation

Neue UI sollte immer zuerst aus der Nutzerentscheidung heraus entworfen werden, nicht aus der Backend-Entität. Die Leitfrage lautet:

> Welche Entscheidung, welches Vertrauen oder welche nächste Aktion unterstützt diese Information?
