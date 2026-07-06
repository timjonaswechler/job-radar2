import { useMemo, useState } from "react";

import { AlertCircleIcon, Code2Icon, XIcon } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { Spinner } from "@/components/ui/spinner";
import {
  SourceAccessPathFields,
  SourceDetectionUrlField,
  SourceIdentityFields,
} from "@/features/sources/add/source-add-form-sections";
import { SourceConfigEditor } from "@/features/sources/add/source-config-editor";
import { SourceDetectionPanel } from "@/features/sources/add/source-detection-panel";
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
} from "@/features/sources/add/source-add-model";
import {
  effectiveSourceConfigSchema,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/add/source-config-schema";
import {
  createSource,
  detectSourceProposalFromUrl,
  type RegistrySource,
  type RegistrySourceProfile,
  type SourceProposalDetectionResult,
} from "@/lib/api/sources";

type SourceAddDrawerProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  onCreated?: () => Promise<unknown> | unknown;
};

export function SourceAddDrawer({
  open,
  onOpenChange,
  profiles,
  sources,
  onCreated,
}: SourceAddDrawerProps) {
  const [url, setUrl] = useState("");
  const [detecting, setDetecting] = useState(false);
  const [detectionResult, setDetectionResult] =
    useState<SourceProposalDetectionResult | null>(null);
  const [detectionError, setDetectionError] = useState<string | null>(null);
  const [form, setForm] = useState<SourceFormState>(emptySourceForm);
  const [keyTouched, setKeyTouched] = useState(false);
  const [configEntries, setConfigEntries] = useState<SourceConfigEntry[]>([]);
  const [jsonPreviewOpen, setJsonPreviewOpen] = useState(false);
  const [saveAttempted, setSaveAttempted] = useState(false);
  const [saving, setSaving] = useState(false);
  const [drawerContentElement, setDrawerContentElement] =
    useState<HTMLDivElement | null>(null);

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
        existingSourceKeys,
        selectedProfile,
        selectedAccessPath,
        schemaMetadata,
      }),
    [
      configEntries,
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

  const updateProfile = (profileKey: string) => {
    const nextDraft = sourceAddDraftAfterProfileChange({
      profiles,
      form,
      configEntries,
      profileKey,
    });
    setForm(nextDraft.form);
    setConfigEntries(nextDraft.configEntries);
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
  };

  const applyDetectedSource = (detected: DetectedSourceLike) => {
    if (saving || detecting) return;

    const nextDraft = sourceAddDraftAfterDetectedSource({ profiles, detected });
    setForm(nextDraft.form);
    setKeyTouched(nextDraft.keyTouched);
    setConfigEntries(nextDraft.configEntries);
    setJsonPreviewOpen(nextDraft.jsonPreviewOpen);
    setSaveAttempted(nextDraft.saveAttempted);
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

  return (
    <Drawer
      open={open}
      onOpenChange={handleOpenChange}
      direction="right"
      handleOnly
    >
      <DrawerContent
        ref={setDrawerContentElement}
        className="h-full data-[vaul-drawer-direction=right]:w-[min(calc(100vw_-_115px),960px)]
      data-[vaul-drawer-direction=right]:sm:max-w-none"
      >
        <DrawerHeader className="border-b pr-12">
          <DrawerTitle>Quelle hinzufügen</DrawerTitle>
          <DrawerDescription>
            Ein Formular für beide Wege: Link prüfen füllt die Felder
            automatisch, manuelle Eingabe füllt dieselben Felder. JSON entsteht
            erst daraus.
          </DrawerDescription>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="absolute top-5 right-5"
            onClick={() => handleOpenChange(false)}
            disabled={asyncActionPending}
          >
            <XIcon aria-hidden="true" />
            <span className="sr-only">Drawer schließen</span>
          </Button>
        </DrawerHeader>

        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          <div className="flex flex-col gap-5">
            <SourceDetectionUrlField
              url={url}
              detectionError={detectionError}
              detecting={detecting}
              saving={saving}
              onUrlChange={setUrl}
              onDetect={handleDetect}
            />

            <SourceDetectionPanel
              result={detectionResult}
              applyDisabled={asyncActionPending}
              onApplyProposal={(proposal) => {
                if (saving || detecting) return;

                const detected = detectedSourceFromProposal(proposal);
                if (detected) applyDetectedSource(detected);
              }}
            />

            <SourceIdentityFields
              form={form}
              saveAttempted={saveAttempted}
              saving={saving}
              selectPortalContainer={drawerContentElement}
              onNameChange={updateName}
              onKeyChange={updateKey}
              onStatusChange={(status) =>
                setForm((current) => ({ ...current, status }))
              }
            />

            <SourceAccessPathFields
              form={form}
              profiles={profiles}
              availableAccessPaths={availableAccessPaths}
              saveAttempted={saveAttempted}
              saving={saving}
              selectPortalContainer={drawerContentElement}
              onProfileChange={updateProfile}
              onAccessPathChange={updateAccessPath}
            />

            <SourceConfigEditor
              entries={configEntries}
              schemaMetadata={schemaMetadata}
              disabled={saving}
              configErrors={buildResult.configErrors}
              showErrors={saveAttempted}
              onChange={setConfigEntries}
            />

            <div className="flex flex-col gap-2">
              <Button
                type="button"
                variant="outline"
                onClick={handlePreviewToggle}
              >
                <Code2Icon data-icon="inline-start" aria-hidden="true" />
                {jsonPreviewOpen ? "JSON ausblenden" : "JSON ansehen"}
              </Button>
              <Collapsible open={jsonPreviewOpen}>
                <CollapsibleContent>
                  <pre className="max-h-96 overflow-auto rounded-md bg-muted p-3 font-mono text-xs">
                    {previewJson}
                  </pre>
                </CollapsibleContent>
              </Collapsible>
            </div>
          </div>
        </div>

        <DrawerFooter className="border-t">
          {saveAttempted && buildResult.errors.length ? (
            <Alert variant="destructive">
              <AlertCircleIcon aria-hidden="true" />
              <AlertTitle>Quelle noch nicht speicherbar</AlertTitle>
              <AlertDescription>
                <ul className="list-inside list-disc">
                  {buildResult.errors.map((error) => (
                    <li key={error}>{error}</li>
                  ))}
                </ul>
              </AlertDescription>
            </Alert>
          ) : null}
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:items-center sm:justify-between">
            <Button
              type="button"
              variant="outline"
              onClick={() => handleOpenChange(false)}
              disabled={asyncActionPending}
            >
              Abbrechen
            </Button>
            <Button
              type="button"
              onClick={handleSave}
              disabled={asyncActionPending}
            >
              {saving ? <Spinner data-icon="inline-start" /> : null}
              Quelle speichern
            </Button>
          </div>
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
