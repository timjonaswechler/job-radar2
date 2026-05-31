import { PageShell } from "./page-shell"

export function DataPage() {
  return (
    <PageShell
      eyebrow="Daten"
      title="Import & Export"
      description="Hier wird später der JSON-Backup- und Restore-Workflow vorbereitet. IDs sollen erhalten bleiben."
      cards={[
        {
          title: "JSON-Export",
          description:
            "Geplant: lokale Daten als transportierbares Backup exportieren.",
          status: "geplant",
        },
        {
          title: "JSON-Import",
          description:
            "Geplant: Wiederherstellung mit stabilen IDs für bestehende Deep Links.",
          status: "geplant",
        },
        {
          title: "Migrations-Backups",
          description:
            "Die Rust-Seite erstellt bereits vor Migrationen eine Datenbankkopie.",
          status: "vorhanden",
        },
      ]}
    />
  )
}
