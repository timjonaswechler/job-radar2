import { useCallback, useEffect, useRef, useState } from "react";

import { toast } from "sonner";

import {
  cancelBackgroundTask,
  getBackgroundTask,
  isInFlightBackgroundTask,
  isTerminalBackgroundTask,
  type BackgroundTaskSnapshot,
} from "@/lib/api/background-tasks";
import {
  parseSearchRunOutcome,
  runSearchRequest,
  type SearchRunOutcome,
} from "@/lib/api/search-runs";

type InFlightTask = BackgroundTaskSnapshot & {
  state: "queued" | "running" | "cancelling";
};
type TerminalTask = BackgroundTaskSnapshot & {
  state: "succeeded" | "failed" | "cancelled";
};

export type SearchRunOperation =
  | { status: "idle" }
  | { status: "starting"; searchRequestId: number; generation: number }
  | {
      status: "active";
      searchRequestId: number;
      generation: number;
      task: InFlightTask;
      cancelling: boolean;
      error: string | null;
    }
  | {
      status: "terminal";
      searchRequestId: number;
      generation: number;
      task: TerminalTask;
      outcome: SearchRunOutcome | null;
      error: string | null;
    }
  | {
      status: "interrupted";
      searchRequestId: number;
      generation: number;
      task: BackgroundTaskSnapshot | null;
      error: string;
    };

type UseSearchRunOptions = {
  onCompleted: () => void | Promise<void>;
};

type OperationIdentity = {
  searchRequestId: number;
  generation: number;
  taskId?: string;
};

export function useSearchRun({ onCompleted }: UseSearchRunOptions) {
  const [operation, setOperation] = useState<SearchRunOperation>({ status: "idle" });
  const operationRef = useRef<SearchRunOperation>(operation);
  const generationRef = useRef(0);
  const responseSequenceRef = useRef(0);
  const mountedRef = useRef(true);
  const onCompletedRef = useRef(onCompleted);
  onCompletedRef.current = onCompleted;

  const transition = useCallback((next: SearchRunOperation) => {
    if (!mountedRef.current) return;
    operationRef.current = next;
    setOperation(next);
  }, []);

  const interrupt = useCallback((
    identity: OperationIdentity,
    message: string,
    task?: BackgroundTaskSnapshot,
  ) => {
    const current = operationRef.current;
    if (!matchesOperation(current, identity)) return;
    transition({
      status: "interrupted",
      searchRequestId: identity.searchRequestId,
      generation: identity.generation,
      task: task ?? (current.status === "active" ? current.task : null),
      error: message,
    });
  }, [transition]);

  const acceptSnapshot = useCallback((
    identity: OperationIdentity,
    task: BackgroundTaskSnapshot,
    response: "start" | "poll" | "cancel",
    responseSequence?: number,
  ) => {
    if (!mountedRef.current) return false;
    const current = operationRef.current;
    if (!matchesOperation(current, identity)) return false;
    if (response === "start" && current.status !== "starting") return false;
    if (response !== "start" && current.status !== "active") return false;

    const staleResponse = responseSequence !== undefined &&
      responseSequence < responseSequenceRef.current;
    if (staleResponse && !isTerminalBackgroundTask(task)) return false;
    if (task.kind !== "search_run") {
      if (staleResponse) return false;
      interrupt(identity, "Der Background Task gehört nicht zu einem Search Run.", task);
      return true;
    }
    if (identity.taskId && task.taskId !== identity.taskId) {
      if (staleResponse) return false;
      interrupt(identity, "Die Background-Task-Antwort gehört nicht zur aktuellen Operation.");
      return true;
    }

    if (isInFlightBackgroundTask(task)) {
      transition({
        status: "active",
        searchRequestId: identity.searchRequestId,
        generation: identity.generation,
        task,
        cancelling: response === "poll" && current.status === "active"
          ? current.cancelling
          : false,
        error: null,
      });
      if (response === "cancel") toast.info("Search Run wird abgebrochen");
      return true;
    }

    if (!isTerminalBackgroundTask(task)) return false;
    const decodedOutcome = parseSearchRunOutcome(task.result);
    const outcome = decodedOutcome?.searchRequestId === identity.searchRequestId
      ? decodedOutcome
      : null;
    if (task.state === "succeeded" && !outcome) {
      interrupt(
        identity,
        "Das Search Run-Ergebnis konnte nicht gelesen oder zugeordnet werden.",
        task,
      );
      return true;
    }

    const error = task.state === "failed"
      ? task.error ?? "Search Run fehlgeschlagen."
      : null;
    transition({
      status: "terminal",
      searchRequestId: identity.searchRequestId,
      generation: identity.generation,
      task,
      outcome,
      error,
    });
    notifyTerminal(task, outcome, error);
    void onCompletedRef.current();
    return true;
  }, [interrupt, transition]);

  const start = useCallback(async (searchRequestId: number, title: string) => {
    const current = operationRef.current;
    if (current.status === "starting" || current.status === "active") return;

    const generation = generationRef.current + 1;
    generationRef.current = generation;
    responseSequenceRef.current = 0;
    const identity = { searchRequestId, generation };
    transition({ status: "starting", ...identity });

    try {
      const task = await runSearchRequest(searchRequestId);
      if (acceptSnapshot(identity, task, "start")) {
        const current = operationRef.current;
        if (current.status === "active" && current.generation === generation) {
          toast.info("Search Run gestartet", { description: title });
        }
      }
    } catch (error) {
      const message = errorMessage(error);
      if (
        !mountedRef.current ||
        !matchesOperation(operationRef.current, identity)
      ) return;
      interrupt(identity, message);
      toast.error("Search Run konnte nicht gestartet werden", {
        description: message,
      });
    }
  }, [acceptSnapshot, interrupt, transition]);

  const cancel = useCallback(async () => {
    const current = operationRef.current;
    if (
      current.status !== "active" ||
      (current.task.state !== "queued" && current.task.state !== "running")
    ) return;

    const identity = {
      searchRequestId: current.searchRequestId,
      generation: current.generation,
      taskId: current.task.taskId,
    };
    const responseSequence = responseSequenceRef.current + 1;
    responseSequenceRef.current = responseSequence;
    transition({ ...current, cancelling: true, error: null });
    try {
      const task = await cancelBackgroundTask(identity.taskId);
      acceptSnapshot(identity, task, "cancel", responseSequence);
    } catch (error) {
      const latest = operationRef.current;
      if (
        !mountedRef.current ||
        responseSequence < responseSequenceRef.current ||
        !matchesOperation(latest, identity) ||
        latest.status !== "active"
      ) return;
      const message = errorMessage(error);
      transition({ ...latest, cancelling: false, error: message });
      toast.error("Search Run konnte nicht abgebrochen werden", {
        description: message,
      });
    }
  }, [acceptSnapshot, transition]);

  useEffect(() => {
    if (operation.status !== "active") return;

    const identity = {
      searchRequestId: operation.searchRequestId,
      generation: operation.generation,
      taskId: operation.task.taskId,
    };
    let disposed = false;
    const timeoutId = window.setTimeout(() => {
      const responseSequence = responseSequenceRef.current + 1;
      responseSequenceRef.current = responseSequence;
      void getBackgroundTask(identity.taskId).then((task) => {
        if (disposed) return;
        acceptSnapshot(identity, task, "poll", responseSequence);
      }).catch((error) => {
        if (
          disposed ||
          responseSequence < responseSequenceRef.current ||
          !matchesOperation(operationRef.current, identity)
        ) return;
        const message = errorMessage(error);
        interrupt(identity, message);
        toast.error("Search Run-Status konnte nicht geladen werden", {
          description: message,
        });
      });
    }, 1000);

    return () => {
      disposed = true;
      window.clearTimeout(timeoutId);
    };
  }, [acceptSnapshot, interrupt, operation, transition]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      generationRef.current += 1;
    };
  }, []);

  return { operation, start, cancel };
}

function matchesOperation(
  operation: SearchRunOperation,
  identity: OperationIdentity,
) {
  if (operation.status === "idle") return false;
  if (
    operation.searchRequestId !== identity.searchRequestId ||
    operation.generation !== identity.generation
  ) return false;
  return !identity.taskId || (
    (operation.status === "active" || operation.status === "terminal") &&
    operation.task.taskId === identity.taskId
  );
}

function notifyTerminal(
  task: BackgroundTaskSnapshot,
  outcome: SearchRunOutcome | null,
  error: string | null,
) {
  if (task.state === "failed") {
    toast.error("Search Run fehlgeschlagen", { description: error ?? undefined });
  } else if (task.state === "cancelled") {
    toast.info("Search Run abgebrochen", { description: task.error ?? undefined });
  } else if (outcome?.status === "completed") {
    toast.success("Search Run abgeschlossen");
  } else if (outcome?.status === "completed_with_errors") {
    toast.warning("Search Run mit Source-Fehlern abgeschlossen");
  } else if (outcome?.status === "cancelled") {
    toast.info("Search Run abgebrochen");
  } else {
    toast.error("Search Run fehlgeschlagen");
  }
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
