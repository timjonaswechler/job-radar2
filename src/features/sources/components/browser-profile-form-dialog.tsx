import { useEffect, useState, type FormEvent } from "react";

import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import type {
  BrowserProfile,
  CreateBrowserProfileInput,
  SourceStatus,
  UpdateBrowserProfileInput,
} from "@/lib/api/sources";
import { sourceStatusOptions } from "@/features/sources/status";

import { JsonField, parseJsonText } from "./json-field";

type BrowserProfileFormDialogProps =
  | {
      open: boolean;
      mode: "create";
      browserProfile?: never;
      onOpenChange: (open: boolean) => void;
      onSubmit: (input: CreateBrowserProfileInput) => Promise<void>;
    }
  | {
      open: boolean;
      mode: "edit";
      browserProfile: BrowserProfile;
      onOpenChange: (open: boolean) => void;
      onSubmit: (input: UpdateBrowserProfileInput) => Promise<void>;
    };

const emptyDefinition = "{}";
const technicalKeyPattern = /^[a-z0-9_]+$/;

export function BrowserProfileFormDialog(props: BrowserProfileFormDialogProps) {
  const { open, mode, onOpenChange } = props;
  const browserProfile = mode === "edit" ? props.browserProfile : null;
  const [key, setKey] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [nameI18nKey, setNameI18nKey] = useState("");
  const [descriptionI18nKey, setDescriptionI18nKey] = useState("");
  const [definitionPath, setDefinitionPath] = useState("");
  const [definitionHash, setDefinitionHash] = useState("");
  const [definitionSchemaVersion, setDefinitionSchemaVersion] = useState("1");
  const [definitionText, setDefinitionText] = useState(emptyDefinition);
  const [sourceConfigSchemaText, setSourceConfigSchemaText] =
    useState(emptyDefinition);
  const [status, setStatus] = useState<SourceStatus>("draft");
  const [validationError, setValidationError] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;

    setKey(browserProfile?.key ?? "");
    setName(browserProfile?.name ?? "");
    setDescription(browserProfile?.description ?? "");
    setNameI18nKey(browserProfile?.nameI18nKey ?? "");
    setDescriptionI18nKey(browserProfile?.descriptionI18nKey ?? "");
    setDefinitionPath(browserProfile?.definitionPath ?? "");
    setDefinitionHash(browserProfile?.definitionHash ?? "");
    setDefinitionSchemaVersion(
      String(browserProfile?.definitionSchemaVersion ?? 1),
    );
    setDefinitionText(formatJsonForField(browserProfile?.definition ?? {}));
    setSourceConfigSchemaText(
      formatJsonForField(browserProfile?.sourceConfigSchema ?? {}),
    );
    setStatus(browserProfile?.status ?? "draft");
    setValidationError(browserProfile?.validationError ?? "");
    setFormError(null);
    setSaving(false);
  }, [browserProfile, open]);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const normalizedKey = key.trim();
    const normalizedName = name.trim();
    const schemaVersion = Number(definitionSchemaVersion);
    const definitionResult = parseJsonText(definitionText);
    const sourceConfigSchemaResult = parseJsonText(sourceConfigSchemaText);

    if (mode === "create" && !technicalKeyPattern.test(normalizedKey)) {
      setFormError(
        "Der Key muss lowercase snake_case mit a-z, 0-9 und _ verwenden.",
      );
      return;
    }

    if (!normalizedName) {
      setFormError("Name darf nicht leer sein.");
      return;
    }

    if (!Number.isInteger(schemaVersion) || schemaVersion < 1) {
      setFormError("Die Profildefinitions-Schemaversion muss größer als 0 sein.");
      return;
    }

    if (!definitionResult.ok) {
      setFormError(`Profildefinition ist kein gültiges JSON: ${definitionResult.error}`);
      return;
    }

    if (!sourceConfigSchemaResult.ok) {
      setFormError(
        `Quellenkonfigurations-Schema ist kein gültiges JSON: ${sourceConfigSchemaResult.error}`,
      );
      return;
    }

    const sharedInput = {
      name: normalizedName,
      description: optionalText(description),
      nameI18nKey: optionalText(nameI18nKey),
      descriptionI18nKey: optionalText(descriptionI18nKey),
      definitionPath: optionalText(definitionPath),
      definitionHash: optionalText(definitionHash),
      definitionSchemaVersion: schemaVersion,
      definition: definitionResult.value,
      sourceConfigSchema: sourceConfigSchemaResult.value,
      status,
      validationError: optionalText(validationError),
    } satisfies UpdateBrowserProfileInput;

    try {
      setSaving(true);
      setFormError(null);
      if (mode === "create") {
        await props.onSubmit({ key: normalizedKey, ...sharedInput });
      } else {
        await props.onSubmit(sharedInput);
      }
      onOpenChange(false);
    } catch (unknownError) {
      setFormError(String(unknownError));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!saving) onOpenChange(nextOpen);
      }}
    >
      <DialogContent className="max-h-[calc(100vh-2rem)] overflow-y-auto sm:max-w-3xl">
        <form className="grid gap-4" onSubmit={(event) => void handleSubmit(event)}>
          <DialogHeader>
            <DialogTitle>
              {mode === "create" ? "Browserprofil anlegen" : "Browserprofil bearbeiten"}
            </DialogTitle>
            <DialogDescription>
              Browserprofile beschreiben wiederverwendbares Website-Verständnis
              und die von Quellen erwartete stabile Konfiguration.
            </DialogDescription>
          </DialogHeader>

          {formError ? (
            <Alert variant="destructive">
              <AlertCircleIcon className="size-4" aria-hidden="true" />
              <AlertTitle>Eingaben konnten nicht gespeichert werden</AlertTitle>
              <AlertDescription>{formError}</AlertDescription>
            </Alert>
          ) : null}

          <div className="grid gap-3 md:grid-cols-2">
            <TextField
              id="browser-profile-key"
              label="Key"
              value={key}
              onChange={setKey}
              disabled={mode === "edit" || saving}
              required={mode === "create"}
              placeholder="stepstone_profile"
              description={
                mode === "edit"
                  ? "Der Key ist nach dem Anlegen unveränderlich."
                  : "Lowercase snake_case, z. B. stepstone_profile."
              }
            />
            <TextField
              id="browser-profile-name"
              label="Name"
              value={name}
              onChange={setName}
              disabled={saving}
              required
              placeholder="StepStone Profil"
            />
            <TextField
              id="browser-profile-name-i18n-key"
              label="Name-i18n-Key"
              value={nameI18nKey}
              onChange={setNameI18nKey}
              disabled={saving}
              placeholder="Optional"
            />
            <TextField
              id="browser-profile-description-i18n-key"
              label="Beschreibungs-i18n-Key"
              value={descriptionI18nKey}
              onChange={setDescriptionI18nKey}
              disabled={saving}
              placeholder="Optional"
            />
            <TextField
              id="browser-profile-definition-path"
              label="Definitionspfad"
              value={definitionPath}
              onChange={setDefinitionPath}
              disabled={saving}
              placeholder="Optional"
            />
            <TextField
              id="browser-profile-definition-hash"
              label="Definitionshash"
              value={definitionHash}
              onChange={setDefinitionHash}
              disabled={saving}
              placeholder="Optional"
            />
            <TextField
              id="browser-profile-schema-version"
              label="Profildefinitions-Schemaversion"
              value={definitionSchemaVersion}
              onChange={setDefinitionSchemaVersion}
              disabled={saving}
              type="number"
              min={1}
              required
            />
            <div className="grid gap-1.5">
              <label className="text-xs font-medium" htmlFor="browser-profile-status">
                Status
              </label>
              <NativeSelect
                id="browser-profile-status"
                className="w-full"
                value={status}
                onChange={(event) => setStatus(event.target.value as SourceStatus)}
                disabled={saving}
              >
                {sourceStatusOptions.map((option) => (
                  <NativeSelectOption key={option.value} value={option.value}>
                    {option.label}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </div>
          </div>

          <div className="grid gap-1.5">
            <label className="text-xs font-medium" htmlFor="browser-profile-description">
              Beschreibung
            </label>
            <Textarea
              id="browser-profile-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              disabled={saving}
              placeholder="Optional"
            />
          </div>

          <JsonField
            id="browser-profile-definition"
            label="Profildefinition JSON"
            value={definitionText}
            onChange={setDefinitionText}
            disabled={saving}
            description="Deklarative Definition des Browserprofils. Keine ausführbaren Skripte oder Zugangsdaten eintragen."
          />

          <JsonField
            id="browser-profile-source-config-schema"
            label="Quellenkonfigurations-Schema JSON"
            value={sourceConfigSchemaText}
            onChange={setSourceConfigSchemaText}
            disabled={saving}
            description="Schema für stabile Quellenkonfigurationen. Suchkriterien gehören nicht hierher."
          />

          <div className="grid gap-1.5">
            <label className="text-xs font-medium" htmlFor="browser-profile-validation-error">
              Validierungsfehler
            </label>
            <Textarea
              id="browser-profile-validation-error"
              value={validationError}
              onChange={(event) => setValidationError(event.target.value)}
              disabled={saving}
              placeholder="Optionaler Diagnosehinweis"
            />
          </div>

          <DialogFooter>
            <button
              type="button"
              className="rounded-md border border-border px-2 py-1 text-xs font-medium hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
              onClick={() => onOpenChange(false)}
              disabled={saving}
            >
              Abbrechen
            </button>
            <button
              type="submit"
              className="rounded-md bg-primary px-2 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/80 disabled:pointer-events-none disabled:opacity-50"
              disabled={saving}
            >
              {saving ? "Speichert…" : "Speichern"}
            </button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

type TextFieldProps = {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  required?: boolean;
  placeholder?: string;
  description?: string;
  type?: string;
  min?: number;
};

function TextField({
  id,
  label,
  value,
  onChange,
  disabled,
  required,
  placeholder,
  description,
  type = "text",
  min,
}: TextFieldProps) {
  return (
    <div className="grid gap-1.5">
      <label className="text-xs font-medium" htmlFor={id}>
        {label}
      </label>
      {description ? (
        <p className="text-xs text-muted-foreground">{description}</p>
      ) : null}
      <Input
        id={id}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        disabled={disabled}
        required={required}
        placeholder={placeholder}
        type={type}
        min={min}
      />
    </div>
  );
}

function optionalText(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function formatJsonForField(value: unknown) {
  return JSON.stringify(value, null, 2);
}
