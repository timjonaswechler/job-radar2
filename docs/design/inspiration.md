# Inspiration Notes

Inspiration wird als Prinzipiensammlung dokumentiert, nicht als 1:1-Kopiervorlage.

Für jede Referenz festhalten:

- Was gefällt?
- Was übernehmen wir nicht?
- Welches übertragbare Prinzip gilt für Job Radar?
- Welche Backend-/Domainkonzepte müssten diese UI tragen?

## Dense dark orders table

Quelle: vom Nutzer bereitgestellter Screenshot einer dunklen Bestell-Tabelle.

### Gefällt

- kompakte, professionelle Data-Grid-Anmutung
- Tabs oben zur groben Segmentierung
- Filterleiste mit Suche, Zeitraum, Status und weiterer Dimension
- „Manage Table“ als explizite Tabellenkonfiguration
- dezenter Tabellenkopf und ruhige Row-Borders
- zwei Badges in einer Status-Spalte
- kleine Metadaten-Pills in Zellen
- Row Actions über `…` statt sichtbarer Aktionsleiste
- ausgewählte/aktive Zeile über subtile Fläche statt starke Farbe

### Nicht übernehmen

- E-Commerce-/Order-Domäne
- exakte Farben oder Branding
- pinke Punktmarker ohne fachliche Bedeutung
- Statusbegriffe wie `Paid`, `Unfulfilled`, `Cancelled`

### Übertragbares Prinzip

Für vergleichbare Arbeitsgegenstände eignet sich eine dichte Data-Grid-Ansicht mit mehreren, aber klar getrennten Statusdimensionen.

Ein einzelnes `OK` ist zu flach. Besser sind explizite Badges, die Backend-Zustände oder sauber abgeleitete Zustände widerspiegeln.

Beispiel für Quellen:

```txt
[Arbeitsstatus] [Registry-Zustand]
[Aktiv] [Bereit]
[Aktiv] [Problem]
[Entwurf] [Bereit]
```

Beispiel für Profile:

```txt
[Profilart] [Registry-Zustand]
[Recruiting-System] [Bereit]
[Website-Familie] [Problem]
```

### Backend-/Domain-Abgleich

Vor Umsetzung prüfen:

- Welche Badge-Dimension kommt direkt aus dem Backend?
- Welche Badge-Dimension ist abgeleitet?
- Bedeutet „Bereit“ nur „keine Registry-Diagnosen“ oder auch „live erfolgreich getestet“?
- Brauchen wir eine andere Bezeichnung, wenn das Backend keine Live-Bereitschaft garantiert?

Siehe [`domain-ui-contract.md`](domain-ui-contract.md).
