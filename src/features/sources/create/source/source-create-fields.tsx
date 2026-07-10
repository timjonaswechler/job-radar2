import { useMemo } from "react";

import { SearchIcon, SparklesIcon } from "lucide-react";

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
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { profileKindLabels } from "@/features/sources/labels";
import {
  SourceNameField,
  SourceStatusField,
} from "@/features/sources/source-form/source-form-fields";
import type {
  ProfileAccessPathDefinition,
  RegistrySourceProfile,
  SourceStatus,
} from "@/lib/api/sources";

import {
  accessPathDisplayName,
  sourceKeyPattern,
  type SourceCreateFormState,
} from "./source-create-model";

type SourceDetectionUrlFieldProps = {
  url: string;
  detectionError: string | null;
  detecting: boolean;
  saving: boolean;
  onUrlChange: (url: string) => void;
  onDetect: () => void;
};

export function SourceDetectionUrlField({
  url,
  detectionError,
  detecting,
  saving,
  onUrlChange,
  onDetect,
}: SourceDetectionUrlFieldProps) {
  return (
    <FieldSet>
      <FieldLegend>Optional: Link prüfen</FieldLegend>
      <FieldGroup>
        <Field data-invalid={Boolean(detectionError) || undefined}>
          <FieldLabel htmlFor="source-detect-url">Link</FieldLabel>
          <InputGroup>
            <InputGroupInput
              id="source-detect-url"
              value={url}
              onChange={(event) => onUrlChange(event.target.value)}
              placeholder="https://firma.example/jobs"
              aria-invalid={Boolean(detectionError) || undefined}
              disabled={detecting || saving}
            />
            <InputGroupAddon align="inline-start">
              <SearchIcon aria-hidden="true" />
            </InputGroupAddon>
            <InputGroupAddon align="inline-end">
              <InputGroupButton
                type="button"
                variant="default"
                onClick={onDetect}
                disabled={detecting || saving}
              >
                {detecting ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <SparklesIcon data-icon="inline-start" aria-hidden="true" />
                )}
                Prüfen
              </InputGroupButton>
            </InputGroupAddon>
          </InputGroup>
          <FieldDescription>
            Die Erkennung prüft vorhandene Quellenprofile und übernimmt
            erkannte Werte in das Formular darunter.
          </FieldDescription>
          {detectionError ? <FieldError>{detectionError}</FieldError> : null}
        </Field>
      </FieldGroup>
    </FieldSet>
  );
}

type SourceCreateIdentityFieldsProps = {
  form: SourceCreateFormState;
  saveAttempted: boolean;
  saving: boolean;
  selectPortalContainer?: HTMLElement | null;
  onNameChange: (name: string) => void;
  onKeyChange: (key: string) => void;
  onStatusChange: (status: SourceStatus) => void;
};

export function SourceCreateIdentityFields({
  form,
  saveAttempted,
  saving,
  selectPortalContainer,
  onNameChange,
  onKeyChange,
  onStatusChange,
}: SourceCreateIdentityFieldsProps) {
  const keyInvalid = saveAttempted && (!form.key || !sourceKeyPattern.test(form.key));

  return (
    <FieldSet>
      <FieldLegend>Quelle</FieldLegend>
      <FieldGroup>
        <SourceNameField
          id="source-create-name"
          name={form.name}
          description="Sichtbarer Name der Quelle in Listen und Suchläufen."
          placeholder="Example Company"
          invalid={saveAttempted && !form.name.trim()}
          disabled={saving}
          onChange={onNameChange}
        />

        <Field data-invalid={keyInvalid || undefined} data-disabled={saving || undefined}>
          <FieldLabel htmlFor="source-create-key">Key</FieldLabel>
          <Input
            id="source-create-key"
            value={form.key}
            onChange={(event) => onKeyChange(event.target.value)}
            placeholder="example_company"
            aria-invalid={keyInvalid || undefined}
            disabled={saving}
          />
          <FieldDescription>
            Wird als Dateiname genutzt: <code>sources/&lt;key&gt;.json</code>.
            Erlaubt sind Kleinbuchstaben, Zahlen und Unterstriche.
          </FieldDescription>
        </Field>

        <SourceStatusField
          status={form.status}
          description="Neue Quellen starten normalerweise als Entwurf, bis du sie geprüft hast."
          disabled={saving}
          selectPortalContainer={selectPortalContainer}
          onChange={onStatusChange}
        />
      </FieldGroup>
    </FieldSet>
  );
}

type SourceAccessPathFieldsProps = {
  form: SourceCreateFormState;
  profiles: RegistrySourceProfile[];
  availableAccessPaths: ProfileAccessPathDefinition[];
  saveAttempted: boolean;
  saving: boolean;
  selectPortalContainer?: HTMLElement | null;
  onProfileChange: (profileKey: string) => void;
  onAccessPathChange: (pathKey: string) => void;
};

export function SourceAccessPathFields({
  form,
  profiles,
  availableAccessPaths,
  saveAttempted,
  saving,
  selectPortalContainer,
  onProfileChange,
  onAccessPathChange,
}: SourceAccessPathFieldsProps) {
  const profileItems = useMemo(
    () =>
      profiles.map((profile) => ({
        value: profile.document.key,
        label: `${profile.document.name} · ${profileKindLabels[profile.document.kind]}`,
      })),
    [profiles],
  );
  const accessPathItems = useMemo(
    () =>
      availableAccessPaths.map((accessPath) => ({
        value: accessPath.key,
        label: accessPathDisplayName(accessPath),
      })),
    [availableAccessPaths],
  );

  return (
    <FieldSet>
      <FieldLegend>Profil und Zugriffspfad</FieldLegend>
      <FieldGroup>
        <Field data-invalid={saveAttempted && !form.profileKey ? true : undefined}>
          <FieldLabel>Quellenprofil</FieldLabel>
          <Select
            items={profileItems}
            modal={false}
            value={form.profileKey || null}
            onValueChange={(value) => {
              if (value) onProfileChange(value);
            }}
          >
            <SelectTrigger
              className="w-full"
              aria-label="Quellenprofil wählen"
              aria-invalid={saveAttempted && !form.profileKey ? true : undefined}
              disabled={!profiles.length || saving}
              data-vaul-no-drag=""
            >
              <SelectValue placeholder="Profil wählen" />
            </SelectTrigger>
            <SelectContent
              alignItemWithTrigger={false}
              portalContainer={selectPortalContainer}
              data-vaul-no-drag=""
            >
              <SelectGroup>
                {profileItems.map(({ value, label }) => (
                  <SelectItem key={value} value={value}>
                    {label}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          <FieldDescription>
            Das Profil bestimmt, welche Zugriffspfade und Konfigurationswerte möglich sind.
          </FieldDescription>
        </Field>

        <Field data-invalid={saveAttempted && !form.pathKey ? true : undefined}>
          <FieldLabel>Zugriffspfad</FieldLabel>
          <Select
            items={accessPathItems}
            modal={false}
            value={form.pathKey || null}
            onValueChange={(value) => {
              if (value) onAccessPathChange(value);
            }}
          >
            <SelectTrigger
              className="w-full"
              aria-label="Zugriffspfad wählen"
              aria-invalid={saveAttempted && !form.pathKey ? true : undefined}
              disabled={!availableAccessPaths.length || saving}
              data-vaul-no-drag=""
            >
              <SelectValue placeholder="Zugriffspfad wählen" />
            </SelectTrigger>
            <SelectContent
              alignItemWithTrigger={false}
              portalContainer={selectPortalContainer}
              data-vaul-no-drag=""
            >
              <SelectGroup>
                {accessPathItems.map(({ value, label }) => (
                  <SelectItem key={value} value={value}>
                    {label}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          <FieldDescription>
            Der Zugriffspfad beschreibt, wie Job Radar Daten von dieser Quelle abruft.
          </FieldDescription>
        </Field>
      </FieldGroup>
    </FieldSet>
  );
}
