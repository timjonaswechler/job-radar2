import { useState } from "react";

import { AlertCircleIcon, Code2Icon, XIcon } from "lucide-react";

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
import { SourceConfigEditor } from "@/features/sources/source-form/source-config/source-config-editor";
import { DiscardSourceChangesDialog } from "@/features/sources/source-form/discard-source-changes-dialog";
import { SourceOverridesEditor } from "@/features/sources/source-form/source-overrides-editor";
import type { RegistrySource, RegistrySourceProfile } from "@/lib/api/sources";

import {
  SourceAccessPathFields,
  SourceCreateIdentityFields,
  SourceDetectionUrlField,
} from "./source-create-fields";
import { SourceDetectionPanel } from "./source-detection-panel";
import { useSourceCreate } from "./use-source-create";

type SourceCreateDrawerProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  onCreated?: () => Promise<unknown> | unknown;
};

export function SourceCreateDrawer({
  open,
  onOpenChange,
  profiles,
  sources,
  onCreated,
}: SourceCreateDrawerProps) {
  const [drawerContentElement, setDrawerContentElement] =
    useState<HTMLDivElement | null>(null);
  const { state, data, actions } = useSourceCreate({
    profiles,
    sources,
    open,
    onCreated,
    onOpenChange,
  });

  return (
    <>
      <Drawer
        open={open}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) actions.requestClose();
        }}
        direction="right"
        handleOnly
      >
        <DrawerContent
        ref={setDrawerContentElement}
        className="h-full data-[vaul-drawer-direction=right]:w-[min(calc(100vw-115px),960px)] data-[vaul-drawer-direction=right]:sm:max-w-none"
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
            onClick={actions.requestClose}
            disabled={state.asyncActionPending}
          >
            <XIcon aria-hidden="true" />
            <span className="sr-only">Drawer schließen</span>
          </Button>
        </DrawerHeader>

        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          <div className="flex flex-col gap-5">
            <SourceDetectionUrlField
              url={state.url}
              detectionError={state.detectionError}
              detecting={state.detecting}
              saving={state.saving}
              onUrlChange={actions.setUrl}
              onDetect={actions.handleDetect}
            />

            <SourceDetectionPanel
              result={state.detectionResult}
              applyDisabled={state.asyncActionPending}
              onApplyProposal={actions.applyProposal}
            />

            <SourceCreateIdentityFields
              form={state.form}
              saveAttempted={state.saveAttempted}
              saving={state.saving}
              selectPortalContainer={drawerContentElement}
              onNameChange={actions.updateName}
              onKeyChange={actions.updateKey}
              onStatusChange={actions.updateStatus}
            />

            <SourceAccessPathFields
              form={state.form}
              profiles={profiles}
              availableAccessPaths={data.availableAccessPaths}
              saveAttempted={state.saveAttempted}
              saving={state.saving}
              selectPortalContainer={drawerContentElement}
              onProfileChange={actions.updateProfile}
              onAccessPathChange={actions.updateAccessPath}
            />

            <SourceConfigEditor
              entries={state.configEntries}
              schemaMetadata={data.schemaMetadata}
              disabled={state.saving}
              configErrors={data.buildResult.configErrors}
              showErrors={state.saveAttempted}
              portalContainer={drawerContentElement}
              onChange={actions.setConfigEntries}
            />

            <SourceOverridesEditor
              value={state.sourceOverridesText}
              disabled={state.saving}
              starterValue={data.sourceOverridesStarter}
              errors={data.buildResult.overridesErrors}
              showErrors={state.saveAttempted}
              onChange={actions.setSourceOverridesText}
            />

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
                  <pre className="max-h-96 overflow-auto rounded-md bg-muted p-3 font-mono text-xs">
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
              onClick={actions.requestClose}
              disabled={state.asyncActionPending}
            >
              Abbrechen
            </Button>
            <Button
              type="button"
              onClick={actions.handleSave}
              disabled={state.asyncActionPending}
            >
              {state.saving ? <Spinner data-icon="inline-start" /> : null}
              Quelle speichern
            </Button>
          </div>
        </DrawerFooter>
        </DrawerContent>
      </Drawer>
      <DiscardSourceChangesDialog
        open={state.discardDialogOpen}
        portalContainer={drawerContentElement}
        onCancel={actions.cancelDiscard}
        onConfirm={actions.confirmDiscard}
      />
    </>
  );
}
