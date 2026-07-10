import { useEffect, useMemo, useState } from "react";

import { toast } from "sonner";

import {
  createSourceConfigEntryId,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import { sourceOverridesStarterForAccessPath } from "@/features/sources/source-form/source-overrides";
import { resolveSource } from "@/features/sources/view-model/registry-resolution";
import {
  updateSource,
  type RegistrySource,
  type RegistrySourceProfile,
  type SourceStatus,
} from "@/lib/api/sources";

import {
  buildUpdatedSourceDocument,
  sourceEditDraftFromSource,
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

  useEffect(() => {
    if (!open || !initialDraft) return;
    setName(initialDraft.name);
    setStatus(initialDraft.status);
    setConfigEntries(initialDraft.configEntries);
    setSourceOverridesText(initialDraft.sourceOverridesText);
    setJsonPreviewOpen(false);
    setSaveAttempted(false);
    setSaving(false);
  }, [initialDraft, open]);

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

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen && saving) return;
    onOpenChange(nextOpen);
  };

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
      onOpenChange(false);
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
      handleOpenChange,
      handlePreviewToggle,
      handleSave,
    },
  };
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
