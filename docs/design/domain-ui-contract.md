# Domain-to-UI Contract

Die UI darf Backend-Daten vereinfachen, aber sie darf die Domäne nicht verfälschen.

Dieses Dokument verbindet die Designregel „nicht alles anzeigen“ mit der Gegenregel „sichtbare UI muss fachlich korrekt aus Backend- und Domainkonzepten abgeleitet sein“.

## Grundsatz

Die UI ist keine 1:1-Abbildung des Backend-Schemas. Sie ist aber eine **semantisch treue Übersetzung** davon.

Das bedeutet:

- Backend-/Domainzustände sind die Quelle der Wahrheit.
- UI-Labels dürfen nutzerfreundlicher sein als technische Namen.
- Abgeleitete UI-Zustände brauchen eine explizite Herleitung.
- Die UI darf keine Zustände erfinden, die das Backend nicht unterscheiden kann.
- Wenn die UI eine Unterscheidung braucht, die das Backend nicht liefern kann, muss das als Produkt-/Backend-Frage markiert werden.

## Vorgehen für neue UI

Vor einer neuen Tabelle, Liste oder Detailansicht:

1. Relevante Domainbegriffe in `CONTEXT.md` prüfen.
2. Relevante API-/Backend-Typen prüfen.
3. Nutzeraufgabe formulieren.
4. Sichtbarkeitsmatrix erstellen.
5. Für jeden sichtbaren Status die Quelle der Wahrheit angeben.

Kurzform:

```txt
UI-Feld | Nutzerlabel | Quelle der Wahrheit | Herleitung | Disclosure-Level
```

## Status-Badges

Status-Badges sind besonders gefährlich, weil sie fachliche Bedeutung verdichten. Jeder Badge braucht:

- eine klare Bedeutung,
- eine Quelle der Wahrheit,
- ein Tooltip oder Detailtext für Unklarheiten,
- eine eindeutige Farbe nach semantischem Zweck.

Nicht gut:

```txt
OK
```

Warum schlecht:

- sagt nicht, was geprüft wurde,
- vermischt Validität, Aktivität und Betriebsbereitschaft,
- ist schwer mit Backend-Zuständen abzugleichen.

Besser:

```txt
Aktiv        # Arbeitsstatus aus Backend
Bereit       # abgeleitet: keine Registry-Diagnosen
Problem      # abgeleitet: mindestens eine relevante Diagnose
Eingebaut    # Herkunft aus Backend
Custom       # Herkunft aus Backend
```

## Beispiel: Quellen

Backend-/API-Felder:

- `SourceDocument.status`: `draft | active | disabled | invalid`
- `RegistrySource.origin`: `built_in | custom`
- `SourceRegistryDiagnostic[]`: Diagnosen zu einem Dokument
- `selectedAccessPath`: Profilbasierter oder quellenspezifischer Zugriffspfad
- Adapter- und Profilauflösung über Registry-Daten

Mögliche UI-Übersetzung:

| UI-Feld | Nutzerlabel | Quelle der Wahrheit | Herleitung | Ebene |
| --- | --- | --- | --- | --- |
| Name | Quellenname | `source.document.name` | direkt | Übersicht |
| Arbeitsstatus | Aktiv/Entwurf/Deaktiviert/Ungültig | `source.document.status` | direkt übersetzt | Zeile |
| Registry-Zustand | Bereit/Problem | `diagnostics.length` | `0 => Bereit`, `>0 => Problem` oder Diagnoseanzahl | Zeile |
| Herkunft | Eingebaut/Custom | `source.origin` | direkt übersetzt | Zeile/Detail |
| Zugriff | Profil / Quellenspezifisch | `selectedAccessPath.type` | direkt übersetzt | Detail |
| Profil/Pfad | Profilname + Pfadname | `selectedAccessPath` + Profile | aufgelöst | Detail |
| Adapter | Laufzeit/Adapter | Adapterauflösung | technisch, nur bei Bedarf | Erweitert/Debug |
| Rohkonfiguration | JSON | `sourceConfig` | direkt | Debug/Rohdaten |

Empfehlung für Tabellenstatus:

```txt
[Arbeitsstatus] [Registry-Zustand]
```

Beispiele:

```txt
[Aktiv] [Bereit]
[Entwurf] [Bereit]
[Aktiv] [Problem]
[Deaktiviert] [Bereit]
[Ungültig] [Problem]
```

## Beispiel: Quellenprofile

Backend-/API-Felder:

- `SourceProfileDocument.kind`: `recruiting_system | job_portal | website_family | generic`
- `RegistrySourceProfile.origin`: `built_in | custom`
- `accessPaths[]`: verfügbare Zugriffspfade
- `SourceRegistryDiagnostic[]`: Diagnosen zu einem Profil
- Adapterauflösung pro Zugriffspfad

Mögliche UI-Übersetzung:

| UI-Feld | Nutzerlabel | Quelle der Wahrheit | Herleitung | Ebene |
| --- | --- | --- | --- | --- |
| Name | Profilname | `profile.document.name` | direkt | Übersicht |
| Profilart | Recruiting-System/Job-Portal/Website-Familie/Generisch | `profile.document.kind` | direkt übersetzt | Zeile |
| Registry-Zustand | Bereit/Problem | `diagnostics.length` | `0 => Bereit`, `>0 => Problem` oder Diagnoseanzahl | Zeile |
| Herkunft | Eingebaut/Custom | `profile.origin` | direkt übersetzt | Zeile/Detail |
| Zugriffspfade | Anzahl/Pfadliste | `accessPaths.length` | direkt/zusammengefasst | Zeile/Detail |
| Adapter | unterstützte Laufzeiten | `accessPaths[].adapterKey` + Adapterregistry | aufgelöst | Erweitert |
| Schema/Detect/Identity | technische Fähigkeit | Profile-Dokument | zusammengefasst, nicht roh | Erweitert/Debug |
| Rohprofil | JSON | Profile-Dokument | direkt | Debug/Rohdaten |

Empfehlung für Tabellenstatus:

```txt
[Profilart oder Herkunft] [Registry-Zustand]
```

Beispiele:

```txt
[Recruiting-System] [Bereit]
[Job-Portal] [Bereit]
[Website-Familie] [Problem]
[Custom] [Problem]
```

Welche erste Badge-Dimension besser ist, hängt vom Screen-Ziel ab:

- Vergleich nach fachlicher Art: `Profilart` zeigen.
- Vergleich nach Installations-/Besitzkontext: `Herkunft` zeigen.
- Debug/Registry-Screen: Herkunft und Diagnose wichtiger.
- Nutzerarbeits-Screen: Profilart und Lesbarkeit wichtiger.

## Wenn UI und Backend nicht zusammenpassen

Wenn ein gewünschter UI-Zustand nicht sauber aus Backend-Daten ableitbar ist, nicht improvisieren.

Stattdessen dokumentieren:

```txt
Gewünschte UI-Aussage:
Warum für Nutzer:innen wichtig:
Aktuell verfügbare Backend-Daten:
Lücke:
Optionen:
- UI vereinfachen
- Backend um expliziten Zustand erweitern
- Zustand im Frontend ableiten und benennen
```

Beispiel:

```txt
Gewünschte UI-Aussage: „Quelle ist einsatzbereit“
Aktuell verfügbar: Arbeitsstatus + Registry-Diagnosen + Adapterauflösung
Lücke: Es gibt keinen ausgeführten Live-Check.
Konsequenz: UI darf „Bereit“ nur als „Dokument ist konsistent“ erklären, nicht als „Quelle funktioniert live“.
```
