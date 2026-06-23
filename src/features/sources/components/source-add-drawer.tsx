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
} from "@/features/sources/components/source-add-form-sections";
import { SourceConfigEditor } from "@/features/sources/components/source-config-editor";
import { SourceDetectionPanel } from "@/features/sources/components/source-detection-panel";
import {
  buildSourceDocument,
  createEntryId,
  detectedSourceFromMatch,
  detectedSourceFromResult,
  emptySourceForm,
  errorMessage,
  technicalKeyFromText,
  type DetectedSourceLike,
  type SourceFormState,
} from "@/features/sources/source-add-model";
import {
  configEntriesFromJsonObject,
  effectiveSourceConfigSchema,
  entriesWithSchemaHints,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/source-config-schema";
import {
  createCustomSource,
  detectSourceFromUrl,
  type RegistrySource,
  type RegistrySourceProfile,
  type SourceDetectionResult,
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
    useState<SourceDetectionResult | null>(null);
  const [detectionError, setDetectionError] = useState<string | null>(null);
  const [form, setForm] = useState<SourceFormState>(emptySourceForm);
  const [keyTouched, setKeyTouched] = useState(false);
  const [configEntries, setConfigEntries] = useState<SourceConfigEntry[]>([]);
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
    availableAccessPaths.find((accessPath) => accessPath.key === form.pathKey) ??
    null;
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
  const previewJson = buildResult.document
    ? JSON.stringify(buildResult.document, null, 2)
    : "";

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

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      resetDrawer();
    }
    onOpenChange(nextOpen);
  };

  const updateName = (name: string) => {
    setForm((current) => ({
      ...current,
      name,
      key: keyTouched ? current.key : technicalKeyFromText(name),
    }));
  };

  const updateKey = (key: string) => {
    setKeyTouched(true);
    setForm((current) => ({ ...current, key: technicalKeyFromText(key) }));
  };

  const updateProfile = (profileKey: string) => {
    const nextProfile =
      profiles.find((profile) => profile.document.key === profileKey) ?? null;
    const nextPathKey = nextProfile?.document.accessPaths[0]?.key ?? "";
    const nextSchema = effectiveSourceConfigSchema(
      nextProfile?.document.sourceConfigSchema,
      nextProfile?.document.accessPaths[0]?.sourceConfigSchema,
    );
    const nextMetadata = sourceConfigSchemaMetadata(nextSchema);

    setForm((current) => ({
      ...current,
      profileKey,
      pathKey: nextPathKey,
    }));
    setConfigEntries((current) => entriesWithSchemaHints(current, nextMetadata));
  };

  const updateAccessPath = (pathKey: string) => {
    const nextPath = availableAccessPaths.find(
      (accessPath) => accessPath.key === pathKey,
    );
    const nextSchema = effectiveSourceConfigSchema(
      selectedProfile?.document.sourceConfigSchema,
      nextPath?.sourceConfigSchema,
    );
    const nextMetadata = sourceConfigSchemaMetadata(nextSchema);

    setForm((current) => ({ ...current, pathKey }));
    setConfigEntries((current) => entriesWithSchemaHints(current, nextMetadata));
  };

  const applyDetectedSource = (detected: DetectedSourceLike) => {
    const nextProfile =
      profiles.find((profile) => profile.document.key === detected.profileKey) ??
      null;
    const nextPath =
      nextProfile?.document.accessPaths.find(
        (accessPath) => accessPath.key === detected.pathKey,
      ) ?? null;
    const nextSchema = effectiveSourceConfigSchema(
      nextProfile?.document.sourceConfigSchema,
      nextPath?.sourceConfigSchema,
    );
    const nextMetadata = sourceConfigSchemaMetadata(nextSchema);

    setForm({
      name: detected.name,
      key: detected.key,
      status: "draft",
      profileKey: detected.profileKey,
      pathKey: detected.pathKey,
    });
    setKeyTouched(false);
    setConfigEntries(
      entriesWithSchemaHints(
        configEntriesFromJsonObject(detected.sourceConfig),
        nextMetadata,
      ),
    );
    setJsonPreviewOpen(false);
    setSaveAttempted(false);
  };

  const handleDetect = async () => {
    const trimmedUrl = url.trim();
    if (!trimmedUrl) {
      setDetectionError("Bitte zuerst einen Link zur Karriere- oder Jobseite eingeben.");
      return;
    }

    try {
      setDetecting(true);
      setDetectionError(null);
      const result = await detectSourceFromUrl(trimmedUrl);
      setDetectionResult(result);

      if (result.status === "detected") {
        const detected = detectedSourceFromResult(result);
        if (detected) {
          applyDetectedSource(detected);
          toast.success("Quelle erkannt und Formular vorausgefüllt.");
        }
      } else if (result.status === "unsupported") {
        setConfigEntries((current) =>
          current.some((entry) => entry.key === "startUrl")
            ? current
            : [
                ...current,
                {
                  id: createEntryId(),
                  key: "startUrl",
                  value: trimmedUrl,
                },
              ],
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
    setSaveAttempted(true);
    if (!buildResult.document) {
      setJsonPreviewOpen(false);
      return;
    }

    try {
      setSaving(true);
      await createCustomSource(buildResult.document);
      try {
        await onCreated?.();
      } catch (refreshError) {
        toast.warning("Quelle gespeichert, Registry konnte aber nicht neu geladen werden.", {
          description: errorMessage(refreshError),
        });
      }
      toast.success("Quelle wurde als Custom-Registry-Dokument gespeichert.");
      handleOpenChange(false);
    } catch (error) {
      toast.error("Quelle konnte nicht gespeichert werden.", {
        description: errorMessage(error),
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <Drawer open={open} onOpenChange={handleOpenChange} direction="right">
      <DrawerContent className="h-full sm:max-w-xl lg:max-w-3xl">
        <DrawerHeader className="border-b pr-12">
          <DrawerTitle>Quelle hinzufügen</DrawerTitle>
          <DrawerDescription>
            Ein Formular für beide Wege: Link prüfen füllt die Felder automatisch,
            manuelle Eingabe füllt dieselben Felder. JSON entsteht erst daraus.
          </DrawerDescription>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="absolute top-5 right-5"
            onClick={() => handleOpenChange(false)}
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
              onApplyMatch={(match) =>
                applyDetectedSource(detectedSourceFromMatch(match))
              }
            />

            <SourceIdentityFields
              form={form}
              saveAttempted={saveAttempted}
              saving={saving}
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
              <Button type="button" variant="outline" onClick={handlePreviewToggle}>
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
              disabled={saving}
            >
              Abbrechen
            </Button>
            <Button type="button" onClick={handleSave} disabled={saving}>
              {saving ? <Spinner data-icon="inline-start" /> : null}
              Quelle speichern
            </Button>
          </div>
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
