import { useMemo, useState } from "react";

import { toast } from "sonner";

import {
  createSource,
  detectSourceProposalFromUrl,
  type RegistrySource,
  type RegistrySourceProfile,
  type SourceProposal,
  type SourceProposalDetectionResult,
  type SourceStatus,
} from "@/lib/api/sources";
import {
  effectiveSourceConfigSchema,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";

import {
  buildSourceDocument,
  detectedSourceFromProposal,
  emptySourceForm,
  errorMessage,
  sourceAddDraftAfterAccessPathChange,
  sourceAddDraftAfterDetectedSource,
  sourceAddDraftAfterDetectionResult,
  sourceAddDraftAfterProfileChange,
  sourceFormAfterKeyChange,
  sourceFormAfterNameChange,
  type DetectedSourceLike,
  type SourceFormState,
} from "./source-add-model";

type UseSourceAddControllerProps = {
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  onCreated?: () => Promise<unknown> | unknown;
  onOpenChange: (open: boolean) => void;
};

export function useSourceAddController({
  profiles,
  sources,
  onCreated,
  onOpenChange,
}: UseSourceAddControllerProps) {
  const [url, setUrl] = useState("");
  const [detecting, setDetecting] = useState(false);
  const [detectionResult, setDetectionResult] =
    useState<SourceProposalDetectionResult | null>(null);
  const [detectionError, setDetectionError] = useState<string | null>(null);
  const [form, setForm] = useState<SourceFormState>(emptySourceForm);
  const [keyTouched, setKeyTouched] = useState(false);
  const [configEntries, setConfigEntries] = useState<SourceConfigEntry[]>([]);
  const [sourceOverridesText, setSourceOverridesText] = useState("");
  const [jsonPreviewOpen, setJsonPreviewOpen] = useState(false);
  const [saveAttempted, setSaveAttempted] = useState(false);
  const [saving, setSaving] = useState(false);

  const existingSourceKeys = useMemo(
    () => new Set(sources.map((source) => source.document.key)),
    [sources],
  );
  const selectedProfile = useMemo(
    () =>
      profiles.find((profile) => profile.document.key === form.profileKey) ??
      null,
    [form.profileKey, profiles],
  );
  const availableAccessPaths = selectedProfile?.document.accessPaths ?? [];
  const selectedAccessPath =
    availableAccessPaths.find(
      (accessPath) => accessPath.key === form.pathKey,
    ) ?? null;
  const sourceConfigSchema = useMemo(
    () =>
      effectiveSourceConfigSchema(
        selectedProfile?.document.sourceConfigSchema,
        selectedAccessPath?.sourceConfigSchema,
      ),
    [selectedAccessPath?.sourceConfigSchema, selectedProfile],
  );
  const schemaMetadata = useMemo(
    () => sourceConfigSchemaMetadata(sourceConfigSchema),
    [sourceConfigSchema],
  );
  const buildResult = useMemo(
    () =>
      buildSourceDocument({
        form,
        configEntries,
        sourceOverridesText,
        existingSourceKeys,
        selectedProfile,
        selectedAccessPath,
        schemaMetadata,
      }),
    [
      configEntries,
      sourceOverridesText,
      existingSourceKeys,
      form,
      schemaMetadata,
      selectedAccessPath,
      selectedProfile,
    ],
  );
  const previewJson = useMemo(
    () =>
      jsonPreviewOpen && buildResult.document
        ? JSON.stringify(buildResult.document, null, 2)
        : "",
    [buildResult.document, jsonPreviewOpen],
  );

  const asyncActionPending = saving || detecting;

  const resetDrawer = () => {
    setUrl("");
    setDetectionResult(null);
    setDetectionError(null);
    setForm(emptySourceForm);
    setKeyTouched(false);
    setConfigEntries([]);
    setSourceOverridesText("");
    setJsonPreviewOpen(false);
    setSaveAttempted(false);
    setSaving(false);
  };

  const handleOpenChange = (nextOpen: boolean, force = false) => {
    if (!nextOpen && asyncActionPending && !force) return;

    if (!nextOpen) {
      resetDrawer();
    }
    onOpenChange(nextOpen);
  };

  const updateName = (name: string) => {
    setForm((current) => sourceFormAfterNameChange(current, keyTouched, name));
  };

  const updateKey = (key: string) => {
    setKeyTouched(true);
    setForm((current) => sourceFormAfterKeyChange(current, key));
  };

  const updateStatus = (status: SourceStatus) => {
    setForm((current) => ({ ...current, status }));
  };

  const updateProfile = (profileKey: string) => {
    const nextDraft = sourceAddDraftAfterProfileChange({
      profiles,
      form,
      configEntries,
      profileKey,
    });
    setForm(nextDraft.form);
    setConfigEntries(nextDraft.configEntries);
    setSourceOverridesText("");
  };

  const updateAccessPath = (pathKey: string) => {
    const nextDraft = sourceAddDraftAfterAccessPathChange({
      selectedProfile,
      form,
      configEntries,
      pathKey,
    });
    setForm(nextDraft.form);
    setConfigEntries(nextDraft.configEntries);
    setSourceOverridesText("");
  };

  const applyDetectedSource = (detected: DetectedSourceLike) => {
    if (saving || detecting) return;

    const nextDraft = sourceAddDraftAfterDetectedSource({ profiles, detected });
    setForm(nextDraft.form);
    setKeyTouched(nextDraft.keyTouched);
    setConfigEntries(nextDraft.configEntries);
    setSourceOverridesText(nextDraft.sourceOverridesText);
    setJsonPreviewOpen(nextDraft.jsonPreviewOpen);
    setSaveAttempted(nextDraft.saveAttempted);
  };

  const applyProposal = (proposal: SourceProposal) => {
    if (saving || detecting) return;

    const detected = detectedSourceFromProposal(proposal);
    if (detected) applyDetectedSource(detected);
  };

  const handleDetect = async () => {
    if (detecting || saving) return;

    const trimmedUrl = url.trim();
    if (!trimmedUrl) {
      setDetectionError(
        "Bitte zuerst einen Link zur Karriere- oder Jobseite eingeben.",
      );
      return;
    }

    try {
      setDetecting(true);
      setDetectionError(null);
      const result = await detectSourceProposalFromUrl(trimmedUrl);
      setDetectionResult(result);

      if (result.status === "matched") {
        const nextDraft = sourceAddDraftAfterDetectionResult({
          draft: {
            form,
            keyTouched,
            configEntries,
            sourceOverridesText,
            jsonPreviewOpen,
            saveAttempted,
          },
          profiles,
          result,
          trimmedUrl,
        });
        if (nextDraft.appliedDetectedSource) {
          setForm(nextDraft.form);
          setKeyTouched(nextDraft.keyTouched);
          setConfigEntries(nextDraft.configEntries);
          setSourceOverridesText(nextDraft.sourceOverridesText);
          setJsonPreviewOpen(nextDraft.jsonPreviewOpen);
          setSaveAttempted(nextDraft.saveAttempted);
          toast.success("Quelle erkannt und Formular vorausgefüllt.");
        }
      } else if (result.status === "unsupported") {
        setConfigEntries((current) =>
          sourceAddDraftAfterDetectionResult({
            draft: {
              form,
              keyTouched,
              configEntries: current,
              sourceOverridesText,
              jsonPreviewOpen,
              saveAttempted,
            },
            profiles,
            result,
            trimmedUrl,
          }).configEntries,
        );
      }
    } catch (error) {
      setDetectionError(errorMessage(error));
    } finally {
      setDetecting(false);
    }
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
    if (saving || detecting) return;

    setSaveAttempted(true);
    if (!buildResult.document) {
      setJsonPreviewOpen(false);
      return;
    }

    try {
      setSaving(true);
      await createSource(buildResult.document);
      try {
        await onCreated?.();
      } catch (refreshError) {
        toast.warning(
          "Quelle gespeichert, Registry konnte aber nicht neu geladen werden.",
          {
            description: errorMessage(refreshError),
          },
        );
      }
      toast.success("Quelle wurde als Custom-Registry-Dokument gespeichert.");
      handleOpenChange(false, true);
    } catch (error) {
      toast.error("Quelle konnte nicht gespeichert werden.", {
        description: errorMessage(error),
      });
    } finally {
      setSaving(false);
    }
  };

  return {
    state: {
      url,
      detecting,
      detectionResult,
      detectionError,
      form,
      configEntries,
      sourceOverridesText,
      jsonPreviewOpen,
      saveAttempted,
      saving,
      asyncActionPending,
    },
    data: {
      availableAccessPaths,
      schemaMetadata,
      buildResult,
      previewJson,
    },
    actions: {
      setUrl,
      setConfigEntries,
      setSourceOverridesText,
      handleOpenChange,
      updateName,
      updateKey,
      updateStatus,
      updateProfile,
      updateAccessPath,
      applyProposal,
      handleDetect,
      handlePreviewToggle,
      handleSave,
    },
  };
}
