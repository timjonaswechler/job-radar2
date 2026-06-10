import { Toaster } from "sonner";

import { StartupNotifications } from "@/app/startup/startup-notifications";

export function AppToaster() {
  return (
    <>
      <Toaster closeButton position="bottom-right" />
      <StartupNotifications />
    </>
  );
}
