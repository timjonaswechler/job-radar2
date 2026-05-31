import { PageShell } from "./page-shell"

export function RemindersPage() {
  return (
    <PageShell
      eyebrow="Planung"
      title="Erinnerungen"
      description="Erinnerungen helfen bei täglichen Suchläufen, Follow-ups, Interviews und eigenen Aufgaben."
      cards={[
        {
          title: "Offene Erinnerungen",
          description:
            "Vorbereitet für eine Liste fälliger und kommender Erinnerungen.",
          status: "geplant",
        },
        {
          title: "Typisierte Aufgaben",
          description:
            "Geplant: Suchlauf starten, Bewerbung nachfassen, Interview und freie Erinnerung.",
          status: "geplant",
        },
        {
          title: "Erledigen",
          description:
            "Erinnerungen sollen als erledigt markiert werden können, ohne die Historie zu verlieren.",
          status: "geplant",
        },
      ]}
    />
  )
}
