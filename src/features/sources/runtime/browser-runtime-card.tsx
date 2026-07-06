import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  AlertCircleIcon,
  CheckCircle2Icon,
  DownloadIcon,
  RefreshCwIcon,
  ShieldCheckIcon,
  Trash2Icon,
} from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge, type BadgeProps } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Progress, ProgressLabel } from "@/components/ui/progress";
import {
  checkBrowserRuntime,
  getBrowserRuntimeStatus,
  installBrowserRuntime,
  listenToBrowserRuntimeInstallProgress,
  uninstallBrowserRuntime,
  type BrowserRuntimeInstallProgress,
  type BrowserRuntimeState,
  type BrowserRuntimeStatus,
} from "@/lib/api/browser-runtime";
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const runtimeStatusLabels: Record<BrowserRuntimeState, string> = {
  unsupported: "Nicht unterstützt",
  notInstalled: "Nicht installiert",
  installing: "Installation läuft",
  installed: "Installiert",
  updateRequired: "Update erforderlich",
  invalid: "Ungültig",
};

const runtimeStatusVariants: Record<
  BrowserRuntimeState,
  BadgeProps["variant"]
> = {
  unsupported: "invert-light",
  notInstalled: "warning-light",
  installing: "info-light",
  installed: "success-light",
  updateRequired: "warning-light",
  invalid: "destructive-light",
};

const progressPhaseLabels: Record<
  BrowserRuntimeInstallProgress["phase"],
  string
> = {
  downloading: "Download",
  verifying: "Prüfung",
  extracting: "Entpacken",
  finalizing: "Abschluss",
  completed: "Abgeschlossen",
  failed: "Fehlgeschlagen",
};

type BusyAction = "refresh" | "install" | "check" | "uninstall" | null;

export function BrowserRuntimeCard() {
  const [status, setStatus] = useState<BrowserRuntimeStatus | null>(null);
  const [progress, setProgress] =
    useState<BrowserRuntimeInstallProgress | null>(null);
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [error, setError] = useState<string | null>(null);
  const [checkMessage, setCheckMessage] = useState<string | null>(null);
  const ctaActionInFlightRef = useRef(false);
  const statusRequestIdRef = useRef(0);

  const loadStatus = useCallback(async () => {
    const requestId = statusRequestIdRef.current + 1;
    statusRequestIdRef.current = requestId;

    setBusyAction((currentAction) => currentAction ?? "refresh");
    setError(null);
    try {
      const nextStatus = await getBrowserRuntimeStatus();
      if (statusRequestIdRef.current !== requestId) return;
      setStatus(nextStatus);
    } catch (unknownError) {
      if (statusRequestIdRef.current !== requestId) return;
      setError(String(unknownError));
    } finally {
      if (statusRequestIdRef.current !== requestId) return;
      setBusyAction((currentAction) =>
        currentAction === "refresh" ? null : currentAction,
      );
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | null = null;
    let reloadTimeoutId: number | null = null;

    void listenToBrowserRuntimeInstallProgress((nextProgress) => {
      if (disposed) return;

      setProgress(nextProgress);
      if (
        nextProgress.phase === "completed" ||
        nextProgress.phase === "failed"
      ) {
        if (reloadTimeoutId !== null) {
          window.clearTimeout(reloadTimeoutId);
        }
        reloadTimeoutId = window.setTimeout(() => {
          reloadTimeoutId = null;
          if (!disposed) void loadStatus();
        }, 250);
      }
    })
      .then((nextUnsubscribe) => {
        if (disposed) {
          nextUnsubscribe();
        } else {
          unsubscribe = nextUnsubscribe;
        }
      })
      .catch((unknownError) => {
        if (!disposed) setError(String(unknownError));
      });

    return () => {
      disposed = true;
      if (reloadTimeoutId !== null) {
        window.clearTimeout(reloadTimeoutId);
      }
      unsubscribe?.();
    };
  }, [loadStatus]);

  const progressPercent = useMemo(() => {
    if (!progress?.totalBytes || progress.totalBytes <= 0) return null;
    return Math.min(
      100,
      Math.round(((progress.downloadedBytes ?? 0) / progress.totalBytes) * 100),
    );
  }, [progress]);

  const installActive =
    busyAction === "install" ||
    status?.status === "installing" ||
    Boolean(
      progress && progress.phase !== "completed" && progress.phase !== "failed",
    );
  const runtimeState = status?.status;
  const canRefresh = !busyAction && !installActive;
  const canInstall =
    Boolean(status) &&
    runtimeState !== "unsupported" &&
    runtimeState !== "installing" &&
    runtimeState !== "installed" &&
    !installActive &&
    !busyAction;
  const canCheck =
    Boolean(status) &&
    runtimeState !== "unsupported" &&
    runtimeState !== "notInstalled" &&
    !installActive &&
    !busyAction;
  const canUninstall =
    Boolean(status) &&
    runtimeState !== "unsupported" &&
    runtimeState !== "notInstalled" &&
    runtimeState !== "installing" &&
    !installActive &&
    !busyAction;

  const installLabel =
    runtimeState === "updateRequired" ? "Aktualisieren" : "Installieren";

  const handleRefresh = async () => {
    if (!canRefresh || ctaActionInFlightRef.current) return;

    ctaActionInFlightRef.current = true;
    try {
      setCheckMessage(null);
      await loadStatus();
    } finally {
      ctaActionInFlightRef.current = false;
    }
  };

  const handleInstall = async () => {
    if (!canInstall || ctaActionInFlightRef.current) return;

    ctaActionInFlightRef.current = true;
    statusRequestIdRef.current += 1;
    try {
      setBusyAction("install");
      setError(null);
      setCheckMessage(null);
      setProgress(null);
      const nextStatus = await installBrowserRuntime();
      setStatus(nextStatus);
      toast.success("Browser-Laufzeit installiert.");
    } catch (unknownError) {
      const message = String(unknownError);
      setError(message);
      toast.error("Browser-Laufzeit konnte nicht installiert werden.", {
        description: message,
      });
    } finally {
      ctaActionInFlightRef.current = false;
      setBusyAction(null);
    }
  };

  const handleCheck = async () => {
    if (!canCheck || ctaActionInFlightRef.current) return;

    ctaActionInFlightRef.current = true;
    statusRequestIdRef.current += 1;
    try {
      setBusyAction("check");
      setError(null);
      const result = await checkBrowserRuntime();
      setStatus(result.status);
      setCheckMessage(result.message);
      if (result.ok) {
        toast.success("Browser-Laufzeit geprüft.");
      } else {
        toast.warning("Browser-Laufzeitprüfung fehlgeschlagen.", {
          description: result.message,
        });
      }
    } catch (unknownError) {
      const message = String(unknownError);
      setError(message);
      toast.error("Browser-Laufzeit konnte nicht geprüft werden.", {
        description: message,
      });
    } finally {
      ctaActionInFlightRef.current = false;
      setBusyAction(null);
    }
  };

  const handleUninstall = async () => {
    if (!canUninstall || ctaActionInFlightRef.current) return;

    ctaActionInFlightRef.current = true;
    statusRequestIdRef.current += 1;
    try {
      setBusyAction("uninstall");
      setError(null);
      setCheckMessage(null);
      const nextStatus = await uninstallBrowserRuntime();
      setStatus(nextStatus);
      setProgress(null);
      toast.success("Browser-Laufzeit entfernt.");
    } catch (unknownError) {
      const message = String(unknownError);
      setError(message);
      toast.error("Browser-Laufzeit konnte nicht entfernt werden.", {
        description: message,
      });
    } finally {
      ctaActionInFlightRef.current = false;
      setBusyAction(null);
    }
  };

  return (
    <Card>
      <CardHeader className="gap-4 px-0 pt-0 sm:flex-row sm:items-start sm:justify-between">
        <div className="grid gap-1.5">
          <CardTitle>Browser-Laufzeit</CardTitle>
          <CardDescription>
            Lokal verwaltete Browser-Installation für browserbasierte Quellen.
          </CardDescription>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => void handleRefresh()}
            disabled={!canRefresh}
          >
            <RefreshCwIcon className="size-4" aria-hidden="true" />
            Aktualisieren
          </Button>
          <Button
            type="button"
            size="sm"
            onClick={() => void handleInstall()}
            disabled={!canInstall}
          >
            <DownloadIcon className="size-4" aria-hidden="true" />
            {busyAction === "install" ? "Installiere…" : installLabel}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => void handleCheck()}
            disabled={!canCheck}
          >
            <ShieldCheckIcon className="size-4" aria-hidden="true" />
            Prüfen
          </Button>
          <Button
            type="button"
            variant="destructive"
            size="sm"
            onClick={() => void handleUninstall()}
            disabled={!canUninstall}
          >
            <Trash2Icon className="size-4" aria-hidden="true" />
            {busyAction === "uninstall" ? "Entferne…" : "Entfernen"}
          </Button>
        </div>
      </CardHeader>

      {error ? (
        <Alert variant="destructive" className="mb-4">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Browser-Laufzeitfehler</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {checkMessage ? (
        <Alert variant="info" className="mb-4">
          <CheckCircle2Icon className="size-4" aria-hidden="true" />
          <AlertTitle>Prüfergebnis</AlertTitle>
          <AlertDescription>{checkMessage}</AlertDescription>
        </Alert>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-[minmax(0,0.75fr)_minmax(0,1.25fr)]">
        <div className="grid gap-3 rounded-md border bg-muted/30 p-3">
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              Status
            </span>
            {status ? (
              <Badge variant={runtimeStatusVariants[status.status]} size="sm">
                {runtimeStatusLabels[status.status]}
              </Badge>
            ) : (
              <Badge variant="outline" size="sm">
                Lädt…
              </Badge>
            )}
          </div>
          {status ? (
            <dl className="grid gap-2 text-sm">
              <RuntimeDetail label="Plattform" value={status.platform} />
              <RuntimeDetail
                label="Erforderliche Version"
                value={status.requiredVersion ?? "—"}
              />
              <RuntimeDetail
                label="Installierte Version"
                value={status.installedVersion ?? "—"}
              />
            </dl>
          ) : (
            <p className="text-sm text-muted-foreground">
              Status wird geladen…
            </p>
          )}
        </div>

        <div className="grid gap-3 rounded-md border bg-muted/30 p-3">
          <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Pfade und Diagnose
          </h3>
          {status ? (
            <dl className="grid gap-2 text-sm">
              <RuntimeDetail
                label="Installationsordner"
                value={status.installDir}
              />
              <RuntimeDetail
                label="Executable"
                value={status.executablePath ?? "—"}
              />
              {status.error ? (
                <RuntimeDetail
                  label="Fehler"
                  value={status.error}
                  tone="danger"
                />
              ) : null}
            </dl>
          ) : (
            <p className="text-sm text-muted-foreground">
              Noch keine Statusdaten vorhanden.
            </p>
          )}
        </div>
      </div>

      {progress ? (
        <div className="mt-4 rounded-md border bg-muted/30 p-3">
          <Progress value={progressPercent}>
            <ProgressLabel>
              Installation: {progressPhaseLabels[progress.phase]}
            </ProgressLabel>
            <span className="ml-auto text-xs/relaxed text-muted-foreground tabular-nums">
              {progressPercent !== null ? `${progressPercent}%` : "—"}
            </span>
          </Progress>
          <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
            {progress.totalBytes ? (
              <span>
                {formatBytes(progress.downloadedBytes ?? 0)} /{" "}
                {formatBytes(progress.totalBytes)}
              </span>
            ) : null}
            {progress.message ? <span>{progress.message}</span> : null}
          </div>
        </div>
      ) : null}
    </Card>
  );
}

function RuntimeDetail({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "danger";
}) {
  return (
    <div className="grid gap-0.5">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd
        className={
          tone === "danger"
            ? "break-words text-destructive"
            : "break-words text-foreground"
        }
      >
        {value}
      </dd>
    </div>
  );
}

function formatBytes(bytes: number) {
  if (bytes <= 0) return "0 B";

  const units = ["B", "KB", "MB", "GB"];
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / 1024 ** exponent;

  return `${value.toFixed(value >= 10 || exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}
