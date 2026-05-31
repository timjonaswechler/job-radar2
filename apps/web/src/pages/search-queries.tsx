import { PageShell } from "./page-shell"

export function SearchQueriesPage() {
  return (
    <PageShell
      eyebrow="Suche"
      title="Suchanfragen"
      description="Suchanfragen bündeln Suchbegriff, Jobquellen, Trefferregeln und Ausschlussbegriffe."
      cards={[
        {
          title: "Suchbegriff und Ort",
          description:
            "Vorbereitet für einfache, wiederverwendbare Suchkriterien ohne komplexe Boolean-Sprache.",
          status: "geplant",
        },
        {
          title: "Trefferregeln",
          description:
            "Geplant: title contains und title does not contain, kombiniert als UND-Regeln.",
          status: "geplant",
        },
        {
          title: "Ausschlussbegriffe",
          description:
            "Geplant: globale und suchspezifische Begriffe, die irrelevante Treffer aus der Inbox halten.",
          status: "geplant",
        },
      ]}
    />
  )
}
