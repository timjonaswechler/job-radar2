import { PageShell } from "./page-shell"

export function SettingsPage() {
  return (
    <PageShell
      eyebrow="App"
      title="Einstellungen"
      description="Platzhalter für lokale App-Einstellungen, ohne Accounts, Profile oder Server-Abhängigkeiten."
      cards={[
        {
          title: "Lokale App",
          description:
            "Job Radar bleibt in Phase 1 desktop-only und single-user.",
          status: "gesetzt",
        },
        {
          title: "Sprache",
          description:
            "Die UI ist auf deutsche Domain-Begriffe ausgelegt; einfache i18n-Struktur ist vorhanden.",
          status: "vorhanden",
        },
        {
          title: "Darstellung",
          description:
            "Theme und Layout werden aktuell über vorhandene UI-Komponenten gesteuert.",
          status: "vorhanden",
        },
      ]}
    />
  )
}
