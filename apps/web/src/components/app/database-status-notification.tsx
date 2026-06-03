import { isTauri, invoke } from "@tauri-apps/api/core"
import { CheckIcon } from "lucide-react"
import { useEffect, useRef } from "react"
import { toast } from "sonner"

import { useLocale } from "@/context/locale-provider-context"
import { Spinner } from "@/components/ui/spinner"

type DatabaseInfo = {
  initializedAt: string | null
}

function LoadingToast({ message }: { message: string }) {
  return (
    <div className="flex w-89 items-center gap-3 rounded-md border border-border bg-popover p-4 text-popover-foreground shadow-lg">
      <Spinner className="size-4 opacity-60" />
      <p className="text-xs font-medium">{message}</p>
    </div>
  )
}

function SuccessToast({
  title,
  description,
}: {
  title: string
  description: string
}) {
  return (
    <div className="flex w-89 items-start gap-3 rounded-md border border-border bg-popover p-4 text-popover-foreground shadow-lg">
      <div className="flex size-4 shrink-0 items-center justify-center rounded-full bg-green-500 text-white">
        <CheckIcon className="size-3" />
      </div>
      <div className="flex flex-1 flex-col gap-0.5">
        <p className="text-xs font-semibold">{title}</p>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
    </div>
  )
}

function formatDatabaseInitializedAt(
  value: string | null,
  formatDateTime: (value: Date | string | number) => string
) {
  if (!value) return "Initialisierungszeit unbekannt."

  return `Datenbank initialisiert: ${formatDateTime(value)}`
}

export function DatabaseStatusNotification() {
  const { formatDateTime } = useLocale()
  const databaseToastId = useRef<string | number>("database-info")

  useEffect(() => {
    if (!isTauri()) return

    let cancelled = false
    const toastId = databaseToastId.current

    toast.custom(
      () => <LoadingToast message="Datenbankstatus wird geladen…" />,
      { id: toastId, duration: Infinity }
    )

    invoke<DatabaseInfo>("get_database_info")
      .then((info) => {
        if (cancelled) return

        toast.custom(
          () => (
            <SuccessToast
              title="Tauri ist verbunden"
              description={formatDatabaseInitializedAt(
                info.initializedAt,
                formatDateTime
              )}
            />
          ),
          { id: toastId, duration: 4000 }
        )
      })
      .catch((error: unknown) => {
        if (cancelled) return

        toast.error("Tauri-Command fehlgeschlagen", {
          id: toastId,
          description: String(error),
          duration: 8000,
        })
      })

    return () => {
      cancelled = true
    }
  }, [formatDateTime])

  return null
}
