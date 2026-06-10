import { isTauri } from "@tauri-apps/api/core";
import { CircleCheckIcon, InfoIcon, TriangleAlertIcon } from "lucide-react";
import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/reui/alert";
import { Spinner } from "@/components/ui/spinner";
import { getDatabaseInfo } from "@/lib/api/database";

const startupToastId = "startup-check";
let hasShownStartupNotification = false;

type StartupToastProps = {
  variant: "info" | "success" | "destructive";
  icon: ReactNode;
  title: string;
  description?: string;
};

function StartupToast({
  variant,
  icon,
  title,
  description,
}: StartupToastProps) {
  return (
    <Alert
      variant={variant}
      className="w-96 max-w-[calc(100vw-2rem)] bg-card/95 p-3 shadow-xl backdrop-blur"
    >
      {icon}
      <AlertTitle>{title}</AlertTitle>
      {description ? <AlertDescription>{description}</AlertDescription> : null}
    </Alert>
  );
}

export function StartupNotifications() {
  const { t } = useTranslation();

  useEffect(() => {
    if (hasShownStartupNotification) return;
    hasShownStartupNotification = true;

    if (!isTauri()) {
      toast.custom(
        () => (
          <StartupToast
            variant="info"
            icon={<InfoIcon />}
            title={t("startup.browserMode.title")}
            description={t("startup.browserMode.description")}
          />
        ),
        { id: startupToastId, duration: 5000 }
      );
      return;
    }

    toast.custom(
      () => (
        <StartupToast
          variant="info"
          icon={<Spinner />}
          title={t("startup.checking")}
        />
      ),
      { id: startupToastId, duration: Infinity }
    );

    void getDatabaseInfo()
      .then((info) => {
        toast.custom(
          () => (
            <StartupToast
              variant="success"
              icon={<CircleCheckIcon />}
              title={t("startup.ready.title")}
              description={t("startup.ready.description", {
                sqliteVersion: info.sqliteVersion,
              })}
            />
          ),
          { id: startupToastId, duration: 5000 }
        );
      })
      .catch((error: unknown) => {
        toast.custom(
          () => (
            <StartupToast
              variant="destructive"
              icon={<TriangleAlertIcon />}
              title={t("startup.failed.title")}
              description={String(error)}
            />
          ),
          { id: startupToastId, duration: 8000 }
        );
      });
  }, [t]);

  return null;
}
