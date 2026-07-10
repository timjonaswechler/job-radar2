import { useCallback, useEffect, useRef, useState } from "react";

import { registerAppNavigationBlocker } from "@/app/navigation/path";

export type UnsavedSourceChangesCloseDecision =
  | "ignore"
  | "close"
  | "confirm";

type UseUnsavedSourceChangesProps = {
  open: boolean;
  isDirty: boolean;
  discardBlocked: boolean;
  onReset: () => void;
  onClose: () => void;
};

export function decideUnsavedSourceChangesClose({
  discardBlocked,
  isDirty,
}: Pick<UseUnsavedSourceChangesProps, "discardBlocked" | "isDirty">): UnsavedSourceChangesCloseDecision {
  if (discardBlocked) return "ignore";
  return isDirty ? "confirm" : "close";
}

export function useUnsavedSourceChanges({
  open,
  isDirty,
  discardBlocked,
  onReset,
  onClose,
}: UseUnsavedSourceChangesProps) {
  const [discardDialogOpen, setDiscardDialogOpen] = useState(false);
  const closeRequestPendingRef = useRef(false);
  const pendingNavigationCommitRef = useRef<(() => void) | null>(null);

  const resetAndClose = useCallback(() => {
    onReset();
    onClose();
  }, [onClose, onReset]);

  const requestCloseWithNavigation = useCallback(
    (navigationCommit: (() => void) | null) => {
      if (closeRequestPendingRef.current) return;

      const decision = decideUnsavedSourceChangesClose({
        discardBlocked,
        isDirty,
      });
      if (decision === "ignore") return;

      if (decision === "close") {
        closeRequestPendingRef.current = true;
        resetAndClose();
        navigationCommit?.();
        return;
      }

      closeRequestPendingRef.current = true;
      pendingNavigationCommitRef.current = navigationCommit;
      setDiscardDialogOpen(true);
    },
    [discardBlocked, isDirty, resetAndClose],
  );

  const requestClose = useCallback(() => {
    requestCloseWithNavigation(null);
  }, [requestCloseWithNavigation]);

  const cancelDiscard = useCallback(() => {
    pendingNavigationCommitRef.current = null;
    closeRequestPendingRef.current = false;
    setDiscardDialogOpen(false);
  }, []);

  const confirmDiscard = useCallback(() => {
    if (!closeRequestPendingRef.current) return;

    const navigationCommit = pendingNavigationCommitRef.current;
    pendingNavigationCommitRef.current = null;
    setDiscardDialogOpen(false);
    resetAndClose();
    navigationCommit?.();
  }, [resetAndClose]);

  const forceCloseAfterSave = useCallback(() => {
    pendingNavigationCommitRef.current = null;
    closeRequestPendingRef.current = true;
    setDiscardDialogOpen(false);
    resetAndClose();
  }, [resetAndClose]);

  useEffect(() => {
    if (open) return;

    pendingNavigationCommitRef.current = null;
    closeRequestPendingRef.current = false;
    setDiscardDialogOpen(false);
  }, [open]);

  useEffect(() => {
    if (!open) return;

    return registerAppNavigationBlocker((commit) => {
      requestCloseWithNavigation(commit);
    });
  }, [open, requestCloseWithNavigation]);

  return {
    isDirty,
    discardDialogOpen,
    requestClose,
    confirmDiscard,
    cancelDiscard,
    forceCloseAfterSave,
  };
}
