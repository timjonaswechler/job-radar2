import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { toast } from "sonner";

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

export type BrowserRuntimeBusyAction =
  | "refresh"
  | "install"
  | "check"
  | "uninstall"
  | null;

export type BrowserRuntimeController = {
  busyAction: BrowserRuntimeBusyAction;
  canCheck: boolean;
  canInstall: boolean;
  canRefresh: boolean;
  canUninstall: boolean;
  checkMessage: string | null;
  error: string | null;
  installActive: boolean;
  loadStatus: () => Promise<void>;
  onCheck: () => Promise<void>;
  onInstall: () => Promise<void>;
  onRefresh: () => Promise<void>;
  onUninstall: () => Promise<void>;
  progress: BrowserRuntimeInstallProgress | null;
  runtimeState: BrowserRuntimeState | undefined;
  status: BrowserRuntimeStatus | null;
};

export function useBrowserRuntimeController(): BrowserRuntimeController {
  const [status, setStatus] = useState<BrowserRuntimeStatus | null>(null);
  const [progress, setProgress] =
    useState<BrowserRuntimeInstallProgress | null>(null);
  const [busyAction, setBusyAction] = useState<BrowserRuntimeBusyAction>(null);
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

  const installActive = useMemo(
    () =>
      busyAction === "install" ||
      status?.status === "installing" ||
      Boolean(
        progress &&
          progress.phase !== "completed" &&
          progress.phase !== "failed",
      ),
    [busyAction, progress, status?.status],
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

  const handleRefresh = useCallback(async () => {
    if (!canRefresh || ctaActionInFlightRef.current) return;

    ctaActionInFlightRef.current = true;
    try {
      setCheckMessage(null);
      await loadStatus();
    } finally {
      ctaActionInFlightRef.current = false;
    }
  }, [canRefresh, loadStatus]);

  const handleInstall = useCallback(async () => {
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
  }, [canInstall]);

  const handleCheck = useCallback(async () => {
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
  }, [canCheck]);

  const handleUninstall = useCallback(async () => {
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
  }, [canUninstall]);

  return {
    busyAction,
    canCheck,
    canInstall,
    canRefresh,
    canUninstall,
    checkMessage,
    error,
    installActive,
    loadStatus,
    onCheck: handleCheck,
    onInstall: handleInstall,
    onRefresh: handleRefresh,
    onUninstall: handleUninstall,
    progress,
    runtimeState,
    status,
  };
}
