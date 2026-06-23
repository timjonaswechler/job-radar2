# Visuelle Richtung

Die visuelle Richtung basiert auf den bereitgestellten Screenshots und den zusammengefassten UI/UX-Videos von Kole Jain.

## Stil

Job Radar soll wirken wie eine ruhige, präzise Desktop-Produktivitätsapp:

- neutral und hochwertig
- informationsdicht, aber geordnet
- tabellen- und detailorientiert
- wenig dekorativ, viel Nutzwert
- kompakte Controls
- subtile Borders statt schwerer Schatten
- Farbe als Signal, nicht als Dekoration

Referenzgefühl:

- Airtable/Notion-Datenbanken für Tabellen und Detailpanels
- Linear/Vercel für ruhige Hierarchie und Neutralität
- Raycast für kompakte Desktop-Interaktion
- moderne shadcn/base UI für Komponenten, Tokens und Dichte

## Aus den Screenshots

Wichtige Muster:

- großer, ruhiger Page Header mit Titel und kurzer Beschreibung
- Tabs direkt über dem Data Grid
- Toolbar mit kleinen Icons für Sortierung, Filter, Suche, Ansicht
- schwarze/neutrale Primary Action, z. B. „Add +“
- Data Grid als zentrale Arbeitsfläche
- Status als weiche Chips
- Detailansicht rechts neben der Tabelle
- Popovers mit klaren Sections, Avataren, Rollen und Aktionen
- große Flächen bleiben weiß/neutral; Farbe erscheint nur punktuell

## Aus den Videos

### Data drives the UI

Die Form folgt den Daten:

- vergleichbare Dinge → Data Grid oder Liste
- zeitliche Dinge → Timeline
- Status → Chip
- Zahlen → rechtsbündig
- komplexe Aktionen → Modal
- einfache Nebenaktionen → Popover

### Progressive Disclosure

Nicht alles gleichzeitig zeigen:

- Standardansicht zeigt nur Entscheidungsdaten
- sekundäre Aktionen hinter Menüs oder Hover
- technische Details einklappen
- Debug-Daten nicht in die Hauptansicht ziehen

### Unsichtbare UI mitdenken

Jeder Screen braucht:

- Empty State
- Loading/Skeleton State
- Error State
- Disabled State
- Hover/Focus/Active State
- Tooltips für reine Icon-Aktionen
- Toasts für direktes Feedback

## Farbe

Farbe soll Bedeutung tragen. Die Brandfarbe darf nicht automatisch überall erscheinen.

Empfohlene Rollen:

- `background`, `card`, `muted`, `border`: dominieren die UI
- `primary`: echte Hauptaktionen
- `brand`: dekorative Identität, Glow, aktive Marker, kleine Highlights
- `accent`: neutraler Hover-/Selection-Hintergrund
- `success`, `warning`, `destructive`, `info`: Status und Feedback

Wenn eine Brandfarbe sehr kräftig ist, darf sie nicht als Flächenfarbe für alle primären Komponenten missbraucht werden. Sonst wirkt die App schnell bunt und unruhig.

## Typografie und Dichte

- wenige Schriftgrößen
- starke Hierarchie über Gewicht, Farbe und Position statt über viele Größen
- Tabellen kompakt halten
- Beschreibungen muted
- keine langen erklärenden Texte in Tabellenzellen
- Details in Panel oder Tooltip auslagern

## Do

- shadcn/reui-Komponenten verwenden
- semantische Tokens verwenden
- Data Grids für vergleichbare Daten
- Detailpanel für Kontext
- Status-Chips statt farbiger Cards
- subtile Linien, kleine Radien, ruhige Flächen
- eine klare primäre Aktion pro Bereich

## Don't

- keine rohen Tailwind-Farben in Produkt-UI, z. B. `text-lime-500`
- keine Backend-Felder ungefiltert anzeigen
- keine großen bunten Dashboard-Karten ohne Entscheidungskontext
- keine technischen Fehlercodes ohne Übersetzung
- keine Brandfarbe als Standard-Hover oder Standard-Card-Hintergrund
- keine Modals für kleine Nebenaktionen
