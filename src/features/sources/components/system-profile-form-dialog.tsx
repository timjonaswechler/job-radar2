import { useEffect, useMemo, useState, type FormEvent } from "react";

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
import {
  formatAdapterOptionLabel,
  sortAdaptersByUserFacingPriority,
} from "@/features/sources/adapter-metadata";
import { sourceStatusOptions } from "@/features/sources/status";
import type {
  AdapterMetadata,
  CreateSystemProfileInput,
  SourceStatus,
  SystemProfile,
  UpdateSystemProfileInput,
} from "@/lib/api/sources";

import { JsonField, parseJsonText } from "./json-field";

type SystemProfileFormDialogProps =
  | {
      open: boolean;
      mode: "create";
      systemProfile?: never;
      adapters: AdapterMetadata[];
      onOpenChange: (open: boolean) => void;
      onSubmit: (input: CreateSystemProfileInput) => Promise<void>;
    }
  | {
      open: boolean;
      mode: "edit";
      systemProfile: SystemProfile;
      adapters: AdapterMetadata[];
      onOpenChange: (open: boolean) => void;
      onSubmit: (input: UpdateSystemProfileInput) => Promise<void>;
    };

const emptyJson = "{}";
const technicalKeyPattern = /^[a-z0-9_]+$/;

export function SystemProfileFormDialog(props: SystemProfileFormDialogProps) {
  const { open, mode, adapters, onOpenChange } = props;
  const systemProfile = mode === "edit" ? props.systemProfile : null;
  const [key, setKey] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [adapterKey, setAdapterKey] = useState("");
  const [definitionSchemaVersion, setDefinitionSchemaVersion] = useState("1");
  const [definitionText, setDefinitionText] = useState(emptyJson);
  const [sourceConfigSchemaText, setSourceConfigSchemaText] = useState(emptyJson);
  const [status, setStatus] = useState<SourceStatus>("draft");
  const [validationError, setValidationError] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const adapterOptions = useMemo(
    () =>
      sortAdaptersByUserFacingPriority(adapters).filter(
        (adapter) => adapter.requiresSystemProfile,
      ),
    [adapters],
  );

  useEffect(() => {
    if (!open) return;

    setKey(systemProfile?.key ?? "");
    setName(systemProfile?.name ?? "");
    setDescription(systemProfile?.description ?? "");
    setAdapterKey(systemProfile?.adapterKey ?? adapterOptions[0]?.key ?? "");
    setDefinitionSchemaVersion(String(systemProfile?.definitionSchemaVersion ?? 1));
    setDefinitionText(formatJsonForField(systemProfile?.definition ?? {}));
    setSourceConfigSchemaText(
      formatJsonForField(systemProfile?.sourceConfigSchema ?? {}),
    );
    setStatus(systemProfile?.status ?? "draft");
    setValidationError(systemProfile?.validationError ?? "");
    setFormError(null);
    setSaving(false);
  }, [adapterOptions, open, systemProfile]);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const normalizedKey = key.trim();
    const normalizedName = name.trim();
    const normalizedAdapterKey = adapterKey.trim();
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

    if (!adapterOptions.some((adapter) => adapter.key === normalizedAdapterKey)) {
      setFormError("Bitte eine deklarative Adapter-Laufzeit wählen.");
      return;
    }

    if (!Number.isInteger(schemaVersion) || schemaVersion < 1) {
      setFormError("Die Definitions-Schemaversion muss größer als 0 sein.");
      return;
    }

    if (!definitionResult.ok) {
      setFormError(`Systemprofildefinition ist kein gültiges JSON: ${definitionResult.error}`);
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
      adapterKey: normalizedAdapterKey,
      definitionSchemaVersion: schemaVersion,
      definition: definitionResult.value,
      sourceConfigSchema: sourceConfigSchemaResult.value,
      status,
      validationError: optionalText(validationError),
    } satisfies UpdateSystemProfileInput;

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
              {mode === "create" ? "Systemprofil anlegen" : "Systemprofil bearbeiten"}
            </DialogTitle>
            <DialogDescription>
              Systemprofile sind ladbare JSON-Definitionen für deterministische
              Systemerkennung und Extraktion. Sie enthalten keine ausführbaren Skripte.
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
              id="system-profile-key"
              label="Key"
              value={key}
              onChange={setKey}
              disabled={mode === "edit" || saving}
              required={mode === "create"}
              placeholder="muz_global_jobboard"
              description={
                mode === "edit"
                  ? "Der Key ist nach dem Anlegen unveränderlich."
                  : "Lowercase snake_case, z. B. muz_global_jobboard."
              }
            />
            <TextField
              id="system-profile-name"
              label="Name"
              value={name}
              onChange={setName}
              disabled={saving}
              required
              placeholder="Milch & Zucker Global Jobboard"
            />
            <div className="grid gap-1.5">
              <label className="text-xs font-medium" htmlFor="system-profile-adapter">
                Adapter-Laufzeit
              </label>
              <NativeSelect
                id="system-profile-adapter"
                className="w-full"
                value={adapterKey}
                onChange={(event) => setAdapterKey(event.target.value)}
                disabled={saving}
                required
              >
                {adapterOptions.map((adapter) => (
                  <NativeSelectOption key={adapter.key} value={adapter.key}>
                    {formatAdapterOptionLabel(adapter)}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </div>
            <TextField
              id="system-profile-schema-version"
              label="Definitions-Schemaversion"
              value={definitionSchemaVersion}
              onChange={setDefinitionSchemaVersion}
              disabled={saving}
              type="number"
              min={1}
              required
            />
            <div className="grid gap-1.5">
              <label className="text-xs font-medium" htmlFor="system-profile-status">
                Status
              </label>
              <NativeSelect
                id="system-profile-status"
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
            <label className="text-xs font-medium" htmlFor="system-profile-description">
              Beschreibung
            </label>
            <Textarea
              id="system-profile-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              disabled={saving}
              placeholder="Optional"
            />
          </div>

          <JsonField
            id="system-profile-definition"
            label="Systemprofildefinition JSON"
            value={definitionText}
            onChange={setDefinitionText}
            disabled={saving}
            description="Deklarative Checks wie htmlContains, fetchText, fetchJson und sourceConfig-Templates. Keine Zugangsdaten eintragen."
          />

          <JsonField
            id="system-profile-source-config-schema"
            label="Quellenkonfigurations-Schema JSON"
            value={sourceConfigSchemaText}
            onChange={setSourceConfigSchemaText}
            disabled={saving}
            description="Zusätzliches Schema für Quellen, die dieses Systemprofil verwenden."
          />

          <div className="grid gap-1.5">
            <label className="text-xs font-medium" htmlFor="system-profile-validation-error">
              Validierungsfehler
            </label>
            <Textarea
              id="system-profile-validation-error"
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
