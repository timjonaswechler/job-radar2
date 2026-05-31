import { PageShell } from "./page-shell"

export function PostingsInboxPage() {
  return (
    <PageShell
      eyebrow="Stellenanzeigen"
      title="Stellenanzeigen-Inbox"
      description="Hier landen neue oder noch nicht entschiedene Stellenanzeigen aus Suchläufen und manuellen Eingaben."
      cards={[
        {
          title: "Filterbare Tabelle",
          description:
            "Vorbereitet für Titel, Firma, Ort, Arbeitsmodell, Status und schnelle Entscheidungen.",
          status: "nächster UI-Schritt",
        },
        {
          title: "Quick Actions",
          description:
            "Geplant: interessant markieren, später ansehen, ausblenden oder in Bewerbung umwandeln.",
          status: "geplant",
        },
        {
          title: "Deduplication-Hinweise",
          description:
            "Später sichtbar: welche Fundstellen zu einer deduplizierten Stellenanzeige gehören.",
          status: "geplant",
        },
      ]}
    />
  )
}
