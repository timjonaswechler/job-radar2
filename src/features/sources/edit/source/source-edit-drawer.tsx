import { useEffect, useMemo, useState } from "react";

import { AlertCircleIcon, Code2Icon, SaveIcon, XIcon } from "lucide-react";
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
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { createEntryId } from "@/features/sources/add/source/source-add-model";
import { SourceConfigEditor } from "@/features/sources/add/source/source-config-editor";
import { SourceOverridesEditor } from "@/features/sources/add/source/source-overrides-editor";
import { sourceOverridesStarterForAccessPath } from "@/features/sources/add/source/source-add-model";
import {
  buildUpdatedSourceDocument,
  sourceEditDraftFromSource,
} from "@/features/sources/edit/source/source-edit-model";
import { sourceStatusOptions } from "@/features/sources/status";
import { sourceConfigSchemaMetadata } from "@/features/sources/shared/source-config-schema";
import { resolveSource } from "@/features/sources/view-model/registry-resolution";
import {
  updateSource,
  type RegistrySource,
  type RegistrySourceProfile,
  type SourceStatus,
} from "@/lib/api/sources";

type SourceEditDrawerProps = {
  source: RegistrySource | null;
  profilesByKey: Map<string, RegistrySourceProfile>;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onUpdated?: () => Promise<unknown> | unknown;
};

export function SourceEditDrawer({
  source,
  profilesByKey,
  open,
  onOpenChange,
  onUpdated,
}: SourceEditDrawerProps) {
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
            createConfigEntryId: createEntryId,
          })
        : null,
    [schemaMetadata, source],
  );

  const [name, setName] = useState(initialDraft?.name ?? "");
  const [status, setStatus] = useState<SourceStatus>(
    initialDraft?.status ?? "draft",
  );
  const [configEntries, setConfigEntries] = useState(
    initialDraft?.configEntries ?? [],
  );
  const [sourceOverridesText, setSourceOverridesText] = useState(
    initialDraft?.sourceOverridesText ?? "",
  );
  const [jsonPreviewOpen, setJsonPreviewOpen] = useState(false);
  const [saveAttempted, setSaveAttempted] = useState(false);
  const [saving, setSaving] = useState(false);
  const [drawerContentElement, setDrawerContentElement] =
    useState<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open || !initialDraft) return;
    setName(initialDraft.name);
    setStatus(initialDraft.status);
    setConfigEntries(initialDraft.configEntries);
    setSourceOverridesText(initialDraft.sourceOverridesText);
    setJsonPreviewOpen(initialDraft.jsonPreviewOpen);
    setSaveAttempted(initialDraft.saveAttempted);
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
    () =>
      sourceOverridesStarterForAccessPath(
        resolution?.profileAccessPath ?? null,
      ),
    [resolution?.profileAccessPath],
  );
  const editable = source?.origin === "custom";
  const supportsProfileOverrides =
    source?.document.selectedAccessPath.type === "profile_access_path";

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

  return (
    <Drawer
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen && saving) return;
        onOpenChange(nextOpen);
      }}
      direction="right"
      handleOnly
    >
      {source ? (
        <DrawerContent
          ref={setDrawerContentElement}
          className="h-full data-[vaul-drawer-direction=right]:w-[min(calc(100vw-115px),960px)] data-[vaul-drawer-direction=right]:sm:max-w-none"
        >
          <DrawerHeader className="border-b pr-12">
            <DrawerTitle>Quelle bearbeiten</DrawerTitle>
            <DrawerDescription>
              Source Key <code>{source.document.key}</code> · gespeichert als
              Custom-Registry-Dokument
            </DrawerDescription>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="absolute top-5 right-5"
              onClick={() => onOpenChange(false)}
              disabled={saving}
            >
              <XIcon aria-hidden="true" />
              <span className="sr-only">Drawer schließen</span>
            </Button>
          </DrawerHeader>

          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            <div className="flex flex-col gap-5">
              {!editable ? (
                <Alert variant="warning">
                  <AlertCircleIcon aria-hidden="true" />
                  <AlertTitle>Eingebaute Source</AlertTitle>
                  <AlertDescription>
                    Eingebaute Sources können in diesem Slice nicht
                    überschrieben werden.
                  </AlertDescription>
                </Alert>
              ) : null}

              <SourceEditIdentityFields
                sourceKey={source.document.key}
                name={name}
                status={status}
                saveAttempted={saveAttempted}
                saving={saving || !editable}
                selectPortalContainer={drawerContentElement}
                onNameChange={setName}
                onStatusChange={setStatus}
              />

              <SourceConfigEditor
                entries={configEntries}
                schemaMetadata={schemaMetadata}
                disabled={saving || !editable}
                configErrors={buildResult.configErrors}
                showErrors={saveAttempted}
                portalContainer={drawerContentElement}
                onChange={setConfigEntries}
              />

              {supportsProfileOverrides ? (
                <SourceOverridesEditor
                  value={sourceOverridesText}
                  disabled={saving || !editable}
                  starterValue={sourceOverridesStarter}
                  errors={buildResult.overridesErrors}
                  showErrors={saveAttempted}
                  onChange={setSourceOverridesText}
                />
              ) : null}

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
                    <pre className="max-h-96 overflow-auto rounded-md p-3 font-mono text-xs">
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
                onClick={() => onOpenChange(false)}
                disabled={saving}
              >
                Abbrechen
              </Button>
              <Button
                type="button"
                onClick={handleSave}
                disabled={saving || !editable}
              >
                {saving ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <SaveIcon data-icon="inline-start" aria-hidden="true" />
                )}
                Änderungen speichern
              </Button>
            </div>
          </DrawerFooter>
        </DrawerContent>
      ) : null}
    </Drawer>
  );
}

type SourceEditIdentityFieldsProps = {
  sourceKey: string;
  name: string;
  status: SourceStatus;
  saveAttempted: boolean;
  saving: boolean;
  selectPortalContainer?: HTMLElement | null;
  onNameChange: (name: string) => void;
  onStatusChange: (status: SourceStatus) => void;
};

function SourceEditIdentityFields({
  sourceKey,
  name,
  status,
  saveAttempted,
  saving,
  selectPortalContainer,
  onNameChange,
  onStatusChange,
}: SourceEditIdentityFieldsProps) {
  return (
    <FieldSet>
      <FieldLegend>Quelle</FieldLegend>
      <FieldGroup>
        <Field data-invalid={saveAttempted && !name.trim() ? true : undefined}>
          <FieldLabel htmlFor="source-edit-name">Name</FieldLabel>
          <Input
            id="source-edit-name"
            value={name}
            onChange={(event) => onNameChange(event.target.value)}
            aria-invalid={saveAttempted && !name.trim() ? true : undefined}
            disabled={saving}
          />
          <FieldDescription>Sichtbarer Name der Quelle.</FieldDescription>
          {saveAttempted && !name.trim() ? (
            <FieldError>Name fehlt.</FieldError>
          ) : null}
        </Field>

        <Field data-disabled>
          <FieldLabel htmlFor="source-edit-key">Key</FieldLabel>
          <Input id="source-edit-key" value={sourceKey} disabled readOnly />
          <FieldDescription>
            Der technische Key bleibt beim Bearbeiten stabil.
          </FieldDescription>
        </Field>

        <Field>
          <FieldLabel>Status</FieldLabel>
          <Select
            items={sourceStatusOptions}
            modal={false}
            value={status}
            onValueChange={(value) => {
              if (value) onStatusChange(value as SourceStatus);
            }}
          >
            <SelectTrigger
              className="w-full"
              aria-label="Status wählen"
              disabled={saving}
              data-vaul-no-drag=""
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent
              alignItemWithTrigger={false}
              portalContainer={selectPortalContainer}
              data-vaul-no-drag=""
            >
              <SelectGroup>
                {sourceStatusOptions.map(({ value, label }) => (
                  <SelectItem key={value} value={value}>
                    {label}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          <FieldDescription>
            Nur aktive und valide Sources werden in Search Runs ausgeführt.
          </FieldDescription>
        </Field>
      </FieldGroup>
    </FieldSet>
  );
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
