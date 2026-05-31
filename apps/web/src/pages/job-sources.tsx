import { PageShell } from "./page-shell"

export function JobSourcesPage() {
  return (
    <PageShell
      eyebrow="Suche"
      title="Jobquellen"
      description="Jobquellen beschreiben konkrete Orte, an denen Job Radar nach Stellenanzeigen suchen darf."
      cards={[
        {
          title: "Quellsystem",
          description:
            "Vorbereitet für Systeme wie Feeds, APIs, Career Pages oder Portale mit eigener Adapterlogik.",
          status: "geplant",
        },
        {
          title: "Schonende Ausführung",
          description:
            "Geplant: Delay, Limits, Retry/Backoff und Stop-on-blocking pro Jobquelle.",
          status: "geplant",
        },
        {
          title: "Aktiv/Inaktiv",
          description:
            "Inaktive Jobquellen bleiben gespeichert, werden aber bei Suchläufen übersprungen.",
          status: "geplant",
        },
      ]}
    />
  )
}
