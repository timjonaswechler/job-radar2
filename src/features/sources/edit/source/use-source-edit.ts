import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { toast } from "sonner";

import {
  createSourceConfigEntryId,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import { sourceOverridesStarterForAccessPath } from "@/features/sources/source-form/source-overrides";
import { useUnsavedSourceChanges } from "@/features/sources/source-form/use-unsaved-source-changes";
import { resolveSource } from "@/features/sources/view-model/registry-resolution";
import {
  updateSource,
  type RegistrySource,
  type RegistrySourceProfile,
  type SourceStatus,
} from "@/lib/api/sources";

import {
  buildUpdatedSourceDocument,
  isSourceEditDraftDirty,
  sourceEditDraftFromSource,
  type SourceEditDraftState,
} from "./source-edit-model";

type UseSourceEditProps = {
  source: RegistrySource | null;
  profilesByKey: Map<string, RegistrySourceProfile>;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onUpdated?: () => Promise<unknown> | unknown;
};

export function useSourceEdit({
  source,
  profilesByKey,
  open,
  onOpenChange,
  onUpdated,
}: UseSourceEditProps) {
  const resolution = useMemo(
    () => (source ? resolveSource(source, profilesByKey) : null),
    [profilesByKey, source],
  );
  const schemaMetadata = useMemo(
    () =>
      sourceConfigSchemaMetadata(resolution?.effectiveSourceConfigSchema ?? {}),
    [resolution?.effectiveSourceConfigSchema],
  );
  const initialDraft = useMemo(
    () =>
      source
        ? sourceEditDraftFromSource({
            source,
            schemaMetadata,
            createConfigEntryId: createSourceConfigEntryId,
          })
        : null,
    [schemaMetadata, source],
  );

  const [name, setName] = useState(initialDraft?.name ?? "");
  const [status, setStatus] = useState<SourceStatus>(
    initialDraft?.status ?? "draft",
  );
  const [configEntries, setConfigEntries] = useState<SourceConfigEntry[]>(
    initialDraft?.configEntries ?? [],
  );
  const [sourceOverridesText, setSourceOverridesText] = useState(
    initialDraft?.sourceOverridesText ?? "",
  );
  const [jsonPreviewOpen, setJsonPreviewOpen] = useState(false);
  const [saveAttempted, setSaveAttempted] = useState(false);
  const [saving, setSaving] = useState(false);
  const sessionSourceKeyRef = useRef<string | null>(
    open ? (source?.document.key ?? null) : null,
  );
  const baselineDraftRef = useRef<SourceEditDraftState | null>(
    open ? initialDraft : null,
  );

  useEffect(() => {
    if (!open) {
      sessionSourceKeyRef.current = null;
      return;
    }
    if (!source || !initialDraft) return;
    if (sessionSourceKeyRef.current === source.document.key) return;

    sessionSourceKeyRef.current = source.document.key;
    baselineDraftRef.current = initialDraft;
    setName(initialDraft.name);
    setStatus(initialDraft.status);
    setConfigEntries(initialDraft.configEntries);
    setSourceOverridesText(initialDraft.sourceOverridesText);
    setJsonPreviewOpen(false);
    setSaveAttempted(false);
    setSaving(false);
  }, [initialDraft, open, source]);

  const buildResult = useMemo(
    () =>
      source
        ? buildUpdatedSourceDocument({
            source,
            name,
            status,
            configEntries,
            sourceOverridesText,
            schemaMetadata,
          })
        : { document: null, errors: [], configErrors: [], overridesErrors: [] },
    [configEntries, name, schemaMetadata, source, sourceOverridesText, status],
  );
  const previewJson = useMemo(
    () =>
      jsonPreviewOpen && buildResult.document
        ? JSON.stringify(buildResult.document, null, 2)
        : "",
    [buildResult.document, jsonPreviewOpen],
  );
  const sourceOverridesStarter = useMemo(
    () => sourceOverridesStarterForAccessPath(resolution?.profileAccessPath ?? null),
    [resolution?.profileAccessPath],
  );
  const editable = source?.origin === "custom";
  const supportsProfileOverrides =
    source?.document.selectedAccessPath.type === "profile_access_path";
  const currentDraft = useMemo<SourceEditDraftState>(
    () => ({ name, status, configEntries, sourceOverridesText }),
    [configEntries, name, sourceOverridesText, status],
  );
  const isDirty = baselineDraftRef.current
    ? isSourceEditDraftDirty(currentDraft, baselineDraftRef.current)
    : false;

  const resetDrawer = useCallback(() => {
    const baselineDraft = baselineDraftRef.current;
    if (baselineDraft) {
      setName(baselineDraft.name);
      setStatus(baselineDraft.status);
      setConfigEntries(baselineDraft.configEntries);
      setSourceOverridesText(baselineDraft.sourceOverridesText);
    }
    setJsonPreviewOpen(false);
    setSaveAttempted(false);
    setSaving(false);
  }, []);

  const closeDrawer = useCallback(() => onOpenChange(false), [onOpenChange]);
  const unsavedChanges = useUnsavedSourceChanges({
    open,
    isDirty,
    discardBlocked: saving,
    onReset: resetDrawer,
    onClose: closeDrawer,
  });

  const handlePreviewToggle = () => {
    if (!buildResult.document) {
      setSaveAttempted(true);
      setJsonPreviewOpen(false);
      return;
    }
    setJsonPreviewOpen((current) => !current);
  };

  const handleSave = async () => {
    if (!source || saving || !editable) return;

    setSaveAttempted(true);
    if (!buildResult.document) {
      setJsonPreviewOpen(false);
      return;
    }

    try {
      setSaving(true);
      await updateSource(buildResult.document);
      try {
        await onUpdated?.();
      } catch (refreshError) {
        toast.warning(
          "Quelle gespeichert, Registry konnte aber nicht neu geladen werden.",
          { description: errorMessage(refreshError) },
        );
      }
      toast.success("Quelle wurde aktualisiert.");
      unsavedChanges.forceCloseAfterSave();
    } catch (error) {
      toast.error("Quelle konnte nicht aktualisiert werden.", {
        description: errorMessage(error),
      });
    } finally {
      setSaving(false);
    }
  };

  return {
    state: {
      name,
      status,
      configEntries,
      sourceOverridesText,
      jsonPreviewOpen,
      saveAttempted,
      saving,
      isDirty: unsavedChanges.isDirty,
      discardDialogOpen: unsavedChanges.discardDialogOpen,
    },
    data: {
      schemaMetadata,
      buildResult,
      previewJson,
      sourceOverridesStarter,
      editable,
      supportsProfileOverrides,
    },
    actions: {
      setName,
      setStatus,
      setConfigEntries,
      setSourceOverridesText,
      requestClose: unsavedChanges.requestClose,
      confirmDiscard: unsavedChanges.confirmDiscard,
      cancelDiscard: unsavedChanges.cancelDiscard,
      handlePreviewToggle,
      handleSave,
    },
  };
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
