import { useState } from "react";

import { AlertCircleIcon, Code2Icon, SaveIcon, XIcon } from "lucide-react";

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
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import {
  SourceNameField,
  SourceStatusField,
} from "@/features/sources/source-form/source-form-fields";
import { SourceConfigEditor } from "@/features/sources/source-form/source-config/source-config-editor";
import { SourceOverridesEditor } from "@/features/sources/source-form/source-overrides-editor";
import type {
  RegistrySource,
  RegistrySourceProfile,
  SourceStatus,
} from "@/lib/api/sources";

import { useSourceEdit } from "./use-source-edit";

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
  const [drawerContentElement, setDrawerContentElement] =
    useState<HTMLDivElement | null>(null);
  const { state, data, actions } = useSourceEdit({
    source,
    profilesByKey,
    open,
    onOpenChange,
    onUpdated,
  });

  return (
    <Drawer
      open={open}
      onOpenChange={actions.handleOpenChange}
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
              onClick={() => actions.handleOpenChange(false)}
              disabled={state.saving}
            >
              <XIcon aria-hidden="true" />
              <span className="sr-only">Drawer schließen</span>
            </Button>
          </DrawerHeader>

          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            <div className="flex flex-col gap-5">
              {!data.editable ? (
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
                name={state.name}
                status={state.status}
                saveAttempted={state.saveAttempted}
                disabled={state.saving || !data.editable}
                selectPortalContainer={drawerContentElement}
                onNameChange={actions.setName}
                onStatusChange={actions.setStatus}
              />

              <SourceConfigEditor
                entries={state.configEntries}
                schemaMetadata={data.schemaMetadata}
                disabled={state.saving || !data.editable}
                configErrors={data.buildResult.configErrors}
                showErrors={state.saveAttempted}
                portalContainer={drawerContentElement}
                onChange={actions.setConfigEntries}
              />

              {data.supportsProfileOverrides ? (
                <SourceOverridesEditor
                  value={state.sourceOverridesText}
                  disabled={state.saving || !data.editable}
                  starterValue={data.sourceOverridesStarter}
                  errors={data.buildResult.overridesErrors}
                  showErrors={state.saveAttempted}
                  onChange={actions.setSourceOverridesText}
                />
              ) : null}

              <div className="flex flex-col gap-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={actions.handlePreviewToggle}
                >
                  <Code2Icon data-icon="inline-start" aria-hidden="true" />
                  {state.jsonPreviewOpen ? "JSON ausblenden" : "JSON ansehen"}
                </Button>
                <Collapsible open={state.jsonPreviewOpen}>
                  <CollapsibleContent>
                    <pre className="max-h-96 overflow-auto rounded-md p-3 font-mono text-xs">
                      {data.previewJson}
                    </pre>
                  </CollapsibleContent>
                </Collapsible>
              </div>
            </div>
          </div>

          <DrawerFooter className="border-t">
            {state.saveAttempted && data.buildResult.errors.length ? (
              <Alert variant="destructive">
                <AlertCircleIcon aria-hidden="true" />
                <AlertTitle>Quelle noch nicht speicherbar</AlertTitle>
                <AlertDescription>
                  <ul className="list-inside list-disc">
                    {data.buildResult.errors.map((error) => (
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
                onClick={() => actions.handleOpenChange(false)}
                disabled={state.saving}
              >
                Abbrechen
              </Button>
              <Button
                type="button"
                onClick={actions.handleSave}
                disabled={state.saving || !data.editable}
              >
                {state.saving ? (
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
  disabled: boolean;
  selectPortalContainer?: HTMLElement | null;
  onNameChange: (name: string) => void;
  onStatusChange: (status: SourceStatus) => void;
};

function SourceEditIdentityFields({
  sourceKey,
  name,
  status,
  saveAttempted,
  disabled,
  selectPortalContainer,
  onNameChange,
  onStatusChange,
}: SourceEditIdentityFieldsProps) {
  return (
    <FieldSet>
      <FieldLegend>Quelle</FieldLegend>
      <FieldGroup>
        <SourceNameField
          id="source-edit-name"
          name={name}
          description="Sichtbarer Name der Quelle."
          invalid={saveAttempted && !name.trim()}
          disabled={disabled}
          onChange={onNameChange}
        />

        <Field data-disabled>
          <FieldLabel htmlFor="source-edit-key">Key</FieldLabel>
          <Input id="source-edit-key" value={sourceKey} disabled readOnly />
          <FieldDescription>
            Der technische Key bleibt beim Bearbeiten stabil.
          </FieldDescription>
        </Field>

        <SourceStatusField
          status={status}
          description="Nur aktive und valide Sources werden in Search Runs ausgeführt."
          disabled={disabled}
          selectPortalContainer={selectPortalContainer}
          onChange={onStatusChange}
        />
      </FieldGroup>
    </FieldSet>
  );
}
