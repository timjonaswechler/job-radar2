import { PageShell } from "./page-shell"

export function NotFoundPage() {
  return (
    <PageShell
      eyebrow="Nicht gefunden"
      title="Diese Ansicht gibt es noch nicht"
      description="Für diesen Pfad ist noch kein Job-Radar-Inhalt registriert. Wähle links einen Bereich aus."
      cards={[]}
    />
  )
}
