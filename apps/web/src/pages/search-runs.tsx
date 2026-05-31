import { PageShell } from "./page-shell"

export function SearchRunsPage() {
  return (
    <PageShell
      eyebrow="Suche"
      title="Suchläufe"
      description="Suchläufe sind bewusste, begrenzte Durchgänge über aktive Suchanfragen und Jobquellen."
      cards={[
        {
          title: "Fortschritt",
          description:
            "Geplant: aktuelle Suchanfrage, aktuelle Jobquelle, Wartezeit, Fundstellen, neue Stellenanzeigen und Fehler.",
          status: "geplant",
        },
        {
          title: "Abbrechen",
          description:
            "Ein laufender Suchlauf soll abbrechbar sein, bereits gefundene Ergebnisse bleiben erhalten.",
          status: "geplant",
        },
        {
          title: "Historie",
          description:
            "Geplant: abgeschlossen, mit Fehlern abgeschlossen, abgebrochen und fehlgeschlagen sichtbar machen.",
          status: "geplant",
        },
      ]}
    />
  )
}
