import { Toaster } from "sonner"

import { DatabaseStatusNotification } from "@/components/app/database-status-notification"

export function AppToaster() {
  return (
    <>
      <DatabaseStatusNotification />
      <Toaster closeButton position="bottom-right" />
    </>
  )
}
