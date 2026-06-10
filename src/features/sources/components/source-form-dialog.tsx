import { useEffect, useMemo, useState, type FormEvent } from "react";

import { AlertCircleIcon, Wand2Icon } from "lucide-react";

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
  formatAdapterCategory,
  formatAdapterOptionLabel,
  formatAdapterRisk,
  sortAdaptersByUserFacingPriority,
} from "@/features/sources/adapter-metadata";
import { sourceStatusOptions } from "@/features/sources/status";
import { detectSourceFromUrl } from "@/lib/api/sources";
import type {
  AdapterMetadata,
  BrowserProfile,
  CreateSourceInput,
  JsonValue,
  Source,
  SourceStatus,
  SystemProfile,
  UpdateSourceInput,
} from "@/lib/api/sources";

import { JsonField, parseJsonText } from "./json-field";

type SourceFormDialogProps =
  | {
      open: boolean;
      mode: "create";
      source?: never;
      browserProfiles: BrowserProfile[];
      systemProfiles: SystemProfile[];
      adapters: AdapterMetadata[];
      onOpenChange: (open: boolean) => void;
      onSubmit: (input: CreateSourceInput) => Promise<void>;
    }
  | {
      open: boolean;
      mode: "edit";
      source: Source;
      browserProfiles: BrowserProfile[];
      systemProfiles: SystemProfile[];
      adapters: AdapterMetadata[];
      onOpenChange: (open: boolean) => void;
      onSubmit: (input: UpdateSourceInput) => Promise<void>;
    };

type JsonObject = { [key: string]: JsonValue };

type SchemaField = {
  key: string;
  label: string;
  required: boolean;
  schema: JsonObject;
};

const technicalKeyPattern = /^[a-z0-9_]+$/;
const defaultSourceConfig = "{}";
const sourceConfigPlaceholder = `{
  "url": "https://example.com/jobs"
}`;

export function SourceFormDialog(props: SourceFormDialogProps) {
  const { open, mode, browserProfiles, systemProfiles, adapters, onOpenChange } = props;
  const source = mode === "edit" ? props.source : null;
  const isBuiltInSource = source?.builtIn ?? false;
  const [key, setKey] = useState("");
  const [adapterKey, setAdapterKey] = useState("");
  const [systemProfileId, setSystemProfileId] = useState("");
  const [browserProfileId, setBrowserProfileId] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [sourceConfigText, setSourceConfigText] = useState(defaultSourceConfig);
  const [status, setStatus] = useState<SourceStatus>("draft");
  const [validationError, setValidationError] = useState("");
  const [urlAssistText, setUrlAssistText] = useState("");
  const [urlAssistMessage, setUrlAssistMessage] = useState<string | null>(null);
  const [urlAssistEvidence, setUrlAssistEvidence] = useState<string[]>([]);
  const [urlAssistError, setUrlAssistError] = useState<string | null>(null);
  const [urlAssistLoading, setUrlAssistLoading] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const adapterOptions = useMemo(() => {
    const options = sortAdaptersByUserFacingPriority(adapters)
      .filter(
        (adapter) =>
          mode === "edit" || !isBuiltInJobPortalAdapterKey(adapter.key),
      )
      .map((adapter) => ({
        key: adapter.key,
        label: formatAdapterOptionLabel(adapter),
        registered: true,
      }));

    if (source && !adapters.some((adapter) => adapter.key === source.adapterKey)) {
      options.push({
        key: source.adapterKey,
        label: `${source.adapterKey} (unregistriert)`,
        registered: false,
      });
    }

    return options;
  }, [adapters, mode, source]);

  const selectedAdapter = useMemo(
    () => adapters.find((adapter) => adapter.key === adapterKey) ?? null,
    [adapters, adapterKey],
  );

  const selectedSystemProfile = useMemo(() => {
    if (!systemProfileId) return null;
    const profileId = Number(systemProfileId);
    return (
      systemProfiles.find((systemProfile) => systemProfile.id === profileId) ??
      null
    );
  }, [systemProfileId, systemProfiles]);

  const selectedBrowserProfile = useMemo(() => {
    if (!browserProfileId) return null;
    const profileId = Number(browserProfileId);
    return (
      browserProfiles.find((browserProfile) => browserProfile.id === profileId) ??
      null
    );
  }, [browserProfileId, browserProfiles]);

  const effectiveSourceConfigSchema = useMemo(() => {
    if (!selectedAdapter) return {};
    let schema = selectedAdapter.sourceConfigSchema;
    if (selectedAdapter.requiresSystemProfile && selectedSystemProfile) {
      schema = mergeJsonObjectSchemas(
        schema,
        selectedSystemProfile.sourceConfigSchema,
      );
    }
    if (selectedAdapter.requiresBrowserProfile && selectedBrowserProfile) {
      schema = mergeJsonObjectSchemas(
        schema,
        selectedBrowserProfile.sourceConfigSchema,
      );
    }
    return schema;
  }, [selectedAdapter, selectedBrowserProfile, selectedSystemProfile]);

  const schemaFields = useMemo(
    () => getSchemaFields(effectiveSourceConfigSchema),
    [effectiveSourceConfigSchema],
  );

  const sourceConfigParseResult = useMemo(
    () => parseJsonText(sourceConfigText),
    [sourceConfigText],
  );

  const sourceConfigObject =
    sourceConfigParseResult.ok && isJsonObject(sourceConfigParseResult.value)
      ? sourceConfigParseResult.value
      : null;

  const schemaValidationMessages = useMemo(() => {
    if (!sourceConfigParseResult.ok) return [];
    return validateConfigAgainstSchema(
      sourceConfigParseResult.value,
      effectiveSourceConfigSchema,
    );
  }, [effectiveSourceConfigSchema, sourceConfigParseResult]);

  useEffect(() => {
    if (!open) return;

    setKey(source?.key ?? "");
    setAdapterKey(source?.adapterKey ?? adapterOptions[0]?.key ?? "");
    setSystemProfileId(
      source?.systemProfileId ? String(source.systemProfileId) : "",
    );
    setBrowserProfileId(
      source?.browserProfileId ? String(source.browserProfileId) : "",
    );
    setName(source?.name ?? "");
    setDescription(source?.description ?? "");
    setSourceConfigText(JSON.stringify(source?.sourceConfig ?? {}, null, 2));
    setStatus(source?.status ?? "draft");
    setValidationError(source?.validationError ?? "");
    setUrlAssistText("");
    setUrlAssistMessage(null);
    setUrlAssistEvidence([]);
    setUrlAssistError(null);
    setUrlAssistLoading(false);
    setFormError(null);
    setSaving(false);
  }, [adapterOptions, open, source]);

  useEffect(() => {
    if (!selectedAdapter?.requiresSystemProfile && systemProfileId) {
      setSystemProfileId("");
    }
    if (!selectedAdapter?.requiresBrowserProfile && browserProfileId) {
      setBrowserProfileId("");
    }
  }, [browserProfileId, selectedAdapter, systemProfileId]);

  const updateSourceConfigField = (fieldKey: string, value: JsonValue | undefined) => {
    const currentResult = parseJsonText(sourceConfigText);
    const currentObject =
      currentResult.ok && isJsonObject(currentResult.value) ? currentResult.value : {};
    const nextObject: JsonObject = { ...currentObject };

    if (value === undefined) {
      delete nextObject[fieldKey];
    } else {
      nextObject[fieldKey] = value;
    }

    setSourceConfigText(JSON.stringify(nextObject, null, 2));
  };

  const handleUrlAssist = async () => {
    setUrlAssistMessage(null);
    setUrlAssistEvidence([]);
    setUrlAssistError(null);

    try {
      setUrlAssistLoading(true);
      const result = await detectSourceFromUrl(urlAssistText);

      if (result.status === "built_in_source") {
        setUrlAssistMessage(result.evidence[0] ?? "Diese Quelle ist bereits eingebaut.");
        return;
      }

      if (result.status === "unsupported") {
        setUrlAssistError(
          "Kein aktives Systemprofil konnte diese URL mit allen Pflichtnachweisen erkennen.",
        );
        return;
      }

      if (result.status === "ambiguous") {
        setUrlAssistError(
          `Mehrere Systemprofile passen (${result.matches
            .map((match) => match.systemProfileKey)
            .join(", ")}). Bitte Systemprofil manuell wählen.`,
        );
        return;
      }

      if (!result.adapterKey || !result.systemProfileId || !result.key || !result.name || !isJsonObject(result.sourceConfig)) {
        setUrlAssistError("Die Erkennung lieferte keinen vollständigen Quellenvorschlag.");
        return;
      }

      setAdapterKey(result.adapterKey);
      setSystemProfileId(String(result.systemProfileId));
      setKey(result.key);
      setName(result.name);
      setBrowserProfileId("");
      setSourceConfigText(JSON.stringify(result.sourceConfig, null, 2));
      setUrlAssistEvidence(result.evidence);
      setUrlAssistMessage(
        `Erkannt: Systemprofil ${result.systemProfileKey}. Bitte Angaben prüfen und speichern.`,
      );
    } catch (unknownError) {
      setUrlAssistError(String(unknownError));
    } finally {
      setUrlAssistLoading(false);
    }
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const normalizedKey = key.trim();
    const normalizedAdapterKey = adapterKey.trim();
    const normalizedName = name.trim();
    const sourceConfigResult = parseJsonText(sourceConfigText);

    if (mode === "create" && !technicalKeyPattern.test(normalizedKey)) {
      setFormError(
        "Der Key muss lowercase snake_case mit a-z, 0-9 und _ verwenden.",
      );
      return;
    }

    if (!technicalKeyPattern.test(normalizedAdapterKey)) {
      setFormError(
        "Der Adapter-Key muss lowercase snake_case mit a-z, 0-9 und _ verwenden.",
      );
      return;
    }

    const adapter = adapters.find(
      (candidateAdapter) => candidateAdapter.key === normalizedAdapterKey,
    );
    if (!adapter) {
      setFormError("Bitte einen registrierten Adapter aus dem Register wählen.");
      return;
    }

    if (!normalizedName) {
      setFormError("Name darf nicht leer sein.");
      return;
    }

    if (!sourceConfigResult.ok) {
      setFormError(
        `Quellenkonfiguration ist kein gültiges JSON: ${sourceConfigResult.error}`,
      );
      return;
    }

    let selectedSystemProfileForSubmit: SystemProfile | null = null;
    if (adapter.requiresSystemProfile) {
      if (!systemProfileId) {
        setFormError("Für diesen Adapter muss ein Systemprofil gewählt werden.");
        return;
      }
      const profileId = Number(systemProfileId);
      selectedSystemProfileForSubmit =
        systemProfiles.find((systemProfile) => systemProfile.id === profileId) ??
        null;
      if (!selectedSystemProfileForSubmit) {
        setFormError("Das gewählte Systemprofil wurde nicht gefunden.");
        return;
      }
      if (selectedSystemProfileForSubmit.adapterKey !== normalizedAdapterKey) {
        setFormError("Das gewählte Systemprofil gehört zu einem anderen Adapter.");
        return;
      }
    } else if (systemProfileId) {
      setFormError("Dieser Adapter verwendet kein Systemprofil.");
      return;
    }

    let selectedProfileForSubmit: BrowserProfile | null = null;
    if (adapter.requiresBrowserProfile) {
      if (!browserProfileId) {
        setFormError("Für diesen Adapter muss ein Browserprofil gewählt werden.");
        return;
      }
      const profileId = Number(browserProfileId);
      selectedProfileForSubmit =
        browserProfiles.find((browserProfile) => browserProfile.id === profileId) ??
        null;
      if (!selectedProfileForSubmit) {
        setFormError("Das gewählte Browserprofil wurde nicht gefunden.");
        return;
      }
    } else if (browserProfileId) {
      setFormError("Dieser Adapter verwendet kein Browserprofil.");
      return;
    }

    let schema = adapter.sourceConfigSchema;
    if (adapter.requiresSystemProfile && selectedSystemProfileForSubmit) {
      schema = mergeJsonObjectSchemas(
        schema,
        selectedSystemProfileForSubmit.sourceConfigSchema,
      );
    }
    if (adapter.requiresBrowserProfile && selectedProfileForSubmit) {
      schema = mergeJsonObjectSchemas(
        schema,
        selectedProfileForSubmit.sourceConfigSchema,
      );
    }
    const configValidationMessages = validateConfigAgainstSchema(
      sourceConfigResult.value,
      schema,
    );
    if (configValidationMessages.length) {
      setFormError(configValidationMessages.join(" "));
      return;
    }

    const sharedInput = {
      adapterKey: normalizedAdapterKey,
      systemProfileId: adapter.requiresSystemProfile ? Number(systemProfileId) : null,
      browserProfileId: adapter.requiresBrowserProfile
        ? Number(browserProfileId)
        : null,
      name: normalizedName,
      description: optionalText(description),
      sourceConfig: sourceConfigResult.value,
      status,
      validationError: optionalText(validationError),
    } satisfies UpdateSourceInput;

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
      <DialogContent className="max-h-[calc(100vh-2rem)] overflow-y-auto sm:max-w-2xl">
        <form className="grid gap-4" onSubmit={(event) => void handleSubmit(event)}>
          <DialogHeader>
            <DialogTitle>
              {mode === "create" ? "Quelle anlegen" : "Quelle bearbeiten"}
            </DialogTitle>
            <DialogDescription>
              Eine Quelle enthält stabile Herkunfts- und Zugriffskonfiguration,
              aber keine Suchkriterien wie Keywords, Rollen, Ort oder Region.
            </DialogDescription>
          </DialogHeader>

          {formError ? (
            <Alert variant="destructive">
              <AlertCircleIcon className="size-4" aria-hidden="true" />
              <AlertTitle>Eingaben konnten nicht gespeichert werden</AlertTitle>
              <AlertDescription>{formError}</AlertDescription>
            </Alert>
          ) : null}

          {mode === "create" ? (
            <section className="grid gap-3 rounded-md border bg-muted/20 p-3">
              <div className="grid gap-1">
                <h3 className="flex items-center gap-2 text-sm font-medium">
                  <Wand2Icon className="size-4 text-muted-foreground" aria-hidden="true" />
                  Quelle per Link anlegen
                </h3>
                <p className="text-xs text-muted-foreground">
                  Karriere- oder Sitemap-URL einfügen. Job Radar schlägt Adapter,
                  Name, Key und Quellenkonfiguration vor. StepStone und Indeed
                  sind bereits als eingebaute Quellen vorhanden.
                </p>
              </div>
              <div className="flex flex-col gap-2 sm:flex-row">
                <Input
                  type="url"
                  value={urlAssistText}
                  onChange={(event) => {
                    setUrlAssistText(event.target.value);
                    setUrlAssistError(null);
                    setUrlAssistMessage(null);
                    setUrlAssistEvidence([]);
                  }}
                  disabled={saving}
                  placeholder="https://join.schott.com/sitemap.xml"
                />
                <button
                  type="button"
                  className="rounded-md border border-border px-2 py-1 text-xs font-medium hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
                  onClick={() => void handleUrlAssist()}
                  disabled={saving || urlAssistLoading}
                >
                  {urlAssistLoading ? "Erkennt…" : "Erkennen"}
                </button>
              </div>
              {urlAssistError ? (
                <p className="text-xs text-destructive">{urlAssistError}</p>
              ) : null}
              {urlAssistMessage ? (
                <p className="text-xs text-muted-foreground">{urlAssistMessage}</p>
              ) : null}
              {urlAssistEvidence.length ? (
                <ul className="grid gap-1 text-xs text-muted-foreground">
                  {urlAssistEvidence.map((evidence) => (
                    <li key={evidence}>✓ {evidence}</li>
                  ))}
                </ul>
              ) : null}
            </section>
          ) : null}

          <div className="grid gap-3 md:grid-cols-2">
            <TextField
              id="source-key"
              label="Key"
              value={key}
              onChange={setKey}
              disabled={mode === "edit" || saving}
              required={mode === "create"}
              placeholder="stepstone"
              description={
                mode === "edit"
                  ? "Der Key ist nach dem Anlegen unveränderlich."
                  : "Lowercase snake_case. Keine Suchbegriffe in den Key aufnehmen."
              }
            />
            <div className="grid gap-1.5">
              <label className="text-xs font-medium" htmlFor="source-adapter-key">
                Adapter
              </label>
              <p className="text-xs text-muted-foreground">
                Karriere-System, Job-Portal oder generischen Adapter wählen. Der technische Key bleibt sichtbar.
              </p>
              <NativeSelect
                id="source-adapter-key"
                className="w-full"
                value={adapterKey}
                onChange={(event) => setAdapterKey(event.target.value)}
                disabled={saving || isBuiltInSource || !adapterOptions.length}
                required
              >
                <NativeSelectOption value="" disabled>
                  Adapter wählen
                </NativeSelectOption>
                {adapterOptions.map((option) => (
                  <NativeSelectOption
                    key={option.key}
                    value={option.key}
                    disabled={!option.registered}
                  >
                    {option.label}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
              {selectedAdapter ? (
                <div className="grid gap-1 text-xs text-muted-foreground">
                  <p>
                    {formatAdapterCategory(selectedAdapter)} · Ausführungsmodus:{" "}
                    {executionModeLabel(selectedAdapter.executionMode)} · Risiko:{" "}
                    {formatAdapterRisk(selectedAdapter)}.
                  </p>
                  <p>{selectedAdapter.description}</p>
                  {selectedAdapter.category === "job_board" ? (
                    <p>
                      Dieser Portaladapter ist für die eingebauten Portale bereits
                      fachlich vorkonfiguriert. Suchtext, Ort und Radius gehören
                      in Suchlauf bzw. Einstellungen, nicht in die Quelle.
                    </p>
                  ) : null}
                  {selectedAdapter.supportsManualRelease ? (
                    <p>
                      Manuelle Freigabe kann nötig sein, z.B. für Cookie-/Captcha-
                      oder Bot-Schutz im Browserprofil.
                    </p>
                  ) : null}
                </div>
              ) : adapterKey ? (
                <p className="text-xs text-destructive">
                  Dieser Adapter ist nicht registriert und muss vor dem Speichern
                  ersetzt werden.
                </p>
              ) : null}
            </div>
            <TextField
              id="source-name"
              label="Name"
              value={name}
              onChange={setName}
              disabled={saving}
              required
              placeholder="StepStone"
            />
            {selectedAdapter?.requiresSystemProfile ? (
              <div className="grid gap-1.5">
                <label className="text-xs font-medium" htmlFor="source-system-profile">
                  Systemprofil
                </label>
                <p className="text-xs text-muted-foreground">
                  Dieser Adapter ist nur die technische Laufzeit; das Systemprofil
                  enthält die deterministischen Prüf- und Extraktionsregeln.
                </p>
                <NativeSelect
                  id="source-system-profile"
                  className="w-full"
                  value={systemProfileId}
                  onChange={(event) => setSystemProfileId(event.target.value)}
                  disabled={saving}
                  required
                >
                  <NativeSelectOption value="" disabled>
                    Systemprofil wählen
                  </NativeSelectOption>
                  {systemProfiles
                    .filter(
                      (systemProfile) =>
                        systemProfile.adapterKey === selectedAdapter.key &&
                        systemProfile.status === "active",
                    )
                    .map((systemProfile) => (
                      <NativeSelectOption
                        key={systemProfile.id}
                        value={String(systemProfile.id)}
                      >
                        {systemProfile.name} ({systemProfile.key})
                      </NativeSelectOption>
                    ))}
                </NativeSelect>
              </div>
            ) : null}
            {selectedAdapter?.requiresBrowserProfile ? (
              <div className="grid gap-1.5">
                <label className="text-xs font-medium" htmlFor="source-browser-profile">
                  Browserprofil
                </label>
                <p className="text-xs text-muted-foreground">
                  Dieser Adapter benötigt ein Browserprofil; dessen Schema steuert
                  die Quellenkonfiguration.
                </p>
                <NativeSelect
                  id="source-browser-profile"
                  className="w-full"
                  value={browserProfileId}
                  onChange={(event) => setBrowserProfileId(event.target.value)}
                  disabled={saving}
                  required
                >
                  <NativeSelectOption value="" disabled>
                    Browserprofil wählen
                  </NativeSelectOption>
                  {browserProfiles.map((browserProfile) => (
                    <NativeSelectOption
                      key={browserProfile.id}
                      value={String(browserProfile.id)}
                    >
                      {browserProfile.name} ({browserProfile.key})
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </div>
            ) : null}
            <div className="grid gap-1.5">
              <label className="text-xs font-medium" htmlFor="source-status">
                Status
              </label>
              <NativeSelect
                id="source-status"
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
            <label className="text-xs font-medium" htmlFor="source-description">
              Beschreibung
            </label>
            <Textarea
              id="source-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              disabled={saving}
              placeholder="Optional"
            />
          </div>

          {isBuiltInSource ? (
            <Alert variant="info">
              <AlertCircleIcon className="size-4" aria-hidden="true" />
              <AlertTitle>Eingebautes Job-Portal</AlertTitle>
              <AlertDescription>
                StepStone und Indeed müssen nicht angelegt oder mit URL-Mustern
                konfiguriert werden. Der Adapter kennt die passenden Links intern;
                hier sind nur optionale Overrides sichtbar.
              </AlertDescription>
            </Alert>
          ) : null}

          <SourceConfigSchemaFields
            disabled={saving}
            fields={schemaFields}
            parseError={
              sourceConfigParseResult.ok ? null : sourceConfigParseResult.error
            }
            sourceConfigObject={sourceConfigObject}
            validationMessages={schemaValidationMessages}
            onFieldChange={updateSourceConfigField}
          />

          <JsonField
            id="source-config"
            label="Quellenkonfiguration JSON (erweitert)"
            value={sourceConfigText}
            onChange={setSourceConfigText}
            disabled={saving}
            placeholder={sourceConfigPlaceholder}
            description="JSON-Fallback für erweiterte oder noch nicht direkt darstellbare Schemafelder. Nur stabile Zugangsparameter eintragen; Suchkriterien gehören in spätere Suchanfragen."
          />

          <div className="grid gap-1.5">
            <label className="text-xs font-medium" htmlFor="source-validation-error">
              Validierungsfehler
            </label>
            <Textarea
              id="source-validation-error"
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

function SourceConfigSchemaFields({
  disabled,
  fields,
  parseError,
  sourceConfigObject,
  validationMessages,
  onFieldChange,
}: {
  disabled: boolean;
  fields: SchemaField[];
  parseError: string | null;
  sourceConfigObject: JsonObject | null;
  validationMessages: string[];
  onFieldChange: (fieldKey: string, value: JsonValue | undefined) => void;
}) {
  return (
    <section className="grid gap-3 rounded-md border bg-muted/20 p-3">
      <div className="grid gap-1">
        <h3 className="text-sm font-medium">Quellenkonfiguration</h3>
        <p className="text-xs text-muted-foreground">
          Felder aus dem Adapter- oder Browserprofil-Schema. Der JSON-Fallback
          darunter bleibt maßgeblich und editierbar.
        </p>
      </div>

      {parseError ? (
        <p className="text-xs text-destructive">
          Direkte Felder sind deaktiviert, weil das JSON ungültig ist: {parseError}
        </p>
      ) : null}

      {!parseError && !sourceConfigObject ? (
        <p className="text-xs text-destructive">
          Direkte Felder erwarten ein JSON-Objekt. Bitte den JSON-Fallback nutzen
          oder ein Objekt eintragen.
        </p>
      ) : null}

      {fields.length && sourceConfigObject ? (
        <div className="grid gap-3 md:grid-cols-2">
          {fields.map((field) => (
            <SchemaConfigField
              key={field.key}
              disabled={disabled || Boolean(parseError) || !sourceConfigObject}
              field={field}
              value={sourceConfigObject[field.key]}
              onChange={(value) => onFieldChange(field.key, value)}
            />
          ))}
        </div>
      ) : null}

      {!fields.length ? (
        <p className="text-xs text-muted-foreground">
          Für das aktuelle Schema sind keine direkten Felder registriert. Die
          Konfiguration kann weiterhin als JSON gepflegt werden.
        </p>
      ) : null}

      {validationMessages.length ? (
        <ul className="grid gap-1 text-xs text-destructive">
          {validationMessages.map((message) => (
            <li key={message}>• {message}</li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}

function SchemaConfigField({
  disabled,
  field,
  value,
  onChange,
}: {
  disabled: boolean;
  field: SchemaField;
  value: JsonValue | undefined;
  onChange: (value: JsonValue | undefined) => void;
}) {
  const schemaType = getSchemaType(field.schema);
  const enumValues = getEnumValues(field.schema);
  const description = getString(field.schema.description);
  const commonLabel = `${field.label}${field.required ? " *" : ""}`;

  if (enumValues.length) {
    const selectedValue = enumValues.some((enumValue) => jsonEquals(enumValue, value))
      ? serializeJsonValue(value)
      : "";

    return (
      <div className="grid gap-1.5">
        <label className="text-xs font-medium" htmlFor={`source-config-${field.key}`}>
          {commonLabel}
        </label>
        {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
        <NativeSelect
          id={`source-config-${field.key}`}
          className="w-full"
          value={selectedValue}
          onChange={(event) => {
            const nextValue = event.target.value;
            onChange(nextValue ? parseSerializedJsonValue(nextValue) : undefined);
          }}
          disabled={disabled}
          required={field.required}
        >
          <NativeSelectOption value="">
            {field.required ? "Wert wählen" : "Nicht gesetzt"}
          </NativeSelectOption>
          {enumValues.map((enumValue, index) => (
            <NativeSelectOption
              key={`${field.key}-${index}`}
              value={serializeJsonValue(enumValue)}
            >
              {formatJsonForDisplay(enumValue)}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </div>
    );
  }

  if (schemaType === "boolean") {
    return (
      <div className="grid gap-1.5">
        <label className="text-xs font-medium" htmlFor={`source-config-${field.key}`}>
          {commonLabel}
        </label>
        {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
        <NativeSelect
          id={`source-config-${field.key}`}
          className="w-full"
          value={typeof value === "boolean" ? String(value) : ""}
          onChange={(event) => {
            const nextValue = event.target.value;
            onChange(nextValue ? nextValue === "true" : undefined);
          }}
          disabled={disabled}
          required={field.required}
        >
          <NativeSelectOption value="">
            {field.required ? "Wert wählen" : "Nicht gesetzt"}
          </NativeSelectOption>
          <NativeSelectOption value="true">Ja</NativeSelectOption>
          <NativeSelectOption value="false">Nein</NativeSelectOption>
        </NativeSelect>
      </div>
    );
  }

  if (schemaType === "number" || schemaType === "integer") {
    return (
      <div className="grid gap-1.5">
        <label className="text-xs font-medium" htmlFor={`source-config-${field.key}`}>
          {commonLabel}
        </label>
        {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
        <Input
          id={`source-config-${field.key}`}
          type="number"
          step={schemaType === "integer" ? "1" : "any"}
          value={typeof value === "number" ? String(value) : ""}
          onChange={(event) => {
            if (!event.target.value) {
              onChange(undefined);
              return;
            }
            const numberValue = Number(event.target.value);
            if (Number.isFinite(numberValue)) onChange(numberValue);
          }}
          disabled={disabled}
          required={field.required}
        />
      </div>
    );
  }

  if (schemaType === "string") {
    const format = getString(field.schema.format);
    const inputType = format === "uri" || format === "url" ? "url" : "text";

    return (
      <div className="grid gap-1.5">
        <label className="text-xs font-medium" htmlFor={`source-config-${field.key}`}>
          {commonLabel}
        </label>
        {description ? <p className="text-xs text-muted-foreground">{description}</p> : null}
        <Input
          id={`source-config-${field.key}`}
          type={inputType}
          value={typeof value === "string" ? value : ""}
          onChange={(event) => {
            const nextValue = event.target.value;
            onChange(nextValue || field.required ? nextValue : undefined);
          }}
          disabled={disabled}
          required={field.required}
        />
      </div>
    );
  }

  return (
    <div className="grid gap-1.5 rounded-md border border-dashed p-3">
      <div className="text-xs font-medium">{commonLabel}</div>
      <p className="text-xs text-muted-foreground">
        Dieses Schemafeld kann nicht direkt dargestellt werden. Bitte den
        JSON-Fallback verwenden.
      </p>
    </div>
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
      />
    </div>
  );
}

function isBuiltInJobPortalAdapterKey(adapterKey: string) {
  return adapterKey === "stepstone_search" || adapterKey === "indeed_search";
}

function mergeJsonObjectSchemas(primarySchema: JsonValue, secondarySchema: JsonValue): JsonValue {
  const primaryObject = asJsonObject(primarySchema);
  const secondaryObject = asJsonObject(secondarySchema);

  if (!primaryObject || !Object.keys(primaryObject).length) return secondarySchema;
  if (!secondaryObject || !Object.keys(secondaryObject).length) return primarySchema;

  const primaryProperties = asJsonObject(primaryObject.properties) ?? {};
  const secondaryProperties = asJsonObject(secondaryObject.properties) ?? {};
  const required = Array.from(
    new Set([
      ...getStringArray(primaryObject.required),
      ...getStringArray(secondaryObject.required),
    ]),
  );

  return {
    ...primaryObject,
    ...secondaryObject,
    type: primaryObject.type ?? secondaryObject.type ?? "object",
    properties: {
      ...primaryProperties,
      ...secondaryProperties,
    },
    ...(required.length ? { required } : {}),
  };
}

function getSchemaFields(schema: JsonValue): SchemaField[] {
  const schemaObject = asJsonObject(schema);
  const properties = asJsonObject(schemaObject?.properties);
  if (!properties) return [];

  const requiredFields = new Set(getStringArray(schemaObject?.required));

  return Object.entries(properties).map(([key, propertySchema]) => {
    const propertySchemaObject = asJsonObject(propertySchema) ?? {};
    return {
      key,
      label: getString(propertySchemaObject.title) ?? key,
      required: requiredFields.has(key),
      schema: propertySchemaObject,
    };
  });
}

function validateConfigAgainstSchema(value: JsonValue, schema: JsonValue) {
  const schemaObject = asJsonObject(schema);
  if (!schemaObject || !Object.keys(schemaObject).length) return [];

  const errors: string[] = [];
  validateSchemaNode(schemaObject, value, "sourceConfig", errors);
  return errors;
}

function validateSchemaNode(
  schema: JsonObject,
  value: JsonValue,
  path: string,
  errors: string[],
) {
  const schemaType = getSchemaType(schema);

  if (schemaType && !valueMatchesSchemaType(value, schemaType)) {
    errors.push(`${path} muss ${schemaTypeLabel(schemaType)} sein.`);
    return;
  }

  const enumValues = getEnumValues(schema);
  if (enumValues.length && !enumValues.some((enumValue) => jsonEquals(enumValue, value))) {
    errors.push(
      `${path} muss einer dieser Werte sein: ${enumValues
        .map(formatJsonForDisplay)
        .join(", ")}.`,
    );
    return;
  }

  if (schemaType === "object" || schema.required !== undefined || schema.properties !== undefined) {
    if (!isJsonObject(value)) {
      errors.push(`${path} muss ein Objekt sein.`);
      return;
    }

    for (const requiredField of getStringArray(schema.required)) {
      if (!(requiredField in value)) {
        errors.push(`${path}.${requiredField} ist erforderlich.`);
      }
    }

    const properties = asJsonObject(schema.properties);
    if (properties) {
      for (const [propertyKey, propertySchema] of Object.entries(properties)) {
        const propertySchemaObject = asJsonObject(propertySchema);
        if (propertySchemaObject && propertyKey in value) {
          validateSchemaNode(
            propertySchemaObject,
            value[propertyKey],
            `${path}.${propertyKey}`,
            errors,
          );
        }
      }
    }
  }

  if (schemaType === "string") {
    const format = getString(schema.format);
    if ((format === "uri" || format === "url") && typeof value === "string") {
      if (!isHttpUrl(value)) {
        errors.push(`${path} muss eine absolute HTTP- oder HTTPS-URL sein.`);
      }
    }
  }

  if ((schemaType === "number" || schemaType === "integer") && typeof value === "number") {
    const minimum = getNumber(schema.minimum);
    if (minimum !== null && value < minimum) {
      errors.push(`${path} muss größer oder gleich ${minimum} sein.`);
    }

    const maximum = getNumber(schema.maximum);
    if (maximum !== null && value > maximum) {
      errors.push(`${path} muss kleiner oder gleich ${maximum} sein.`);
    }
  }
}

function valueMatchesSchemaType(value: JsonValue, schemaType: string) {
  if (schemaType === "string") return typeof value === "string";
  if (schemaType === "boolean") return typeof value === "boolean";
  if (schemaType === "number") return typeof value === "number";
  if (schemaType === "integer") return typeof value === "number" && Number.isInteger(value);
  if (schemaType === "object") return isJsonObject(value);
  if (schemaType === "array") return Array.isArray(value);
  return true;
}

function schemaTypeLabel(schemaType: string) {
  if (schemaType === "string") return "ein Text";
  if (schemaType === "boolean") return "ein Wahr/Falsch-Wert";
  if (schemaType === "number") return "eine Zahl";
  if (schemaType === "integer") return "eine ganze Zahl";
  if (schemaType === "object") return "ein Objekt";
  if (schemaType === "array") return "eine Liste";
  return "vom erwarteten Typ";
}

function isJsonObject(value: JsonValue): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function asJsonObject(value: JsonValue | undefined): JsonObject | null {
  return value !== undefined && isJsonObject(value) ? value : null;
}

function getSchemaType(schema: JsonObject) {
  const typeValue = schema.type;
  return typeof typeValue === "string" ? typeValue : null;
}

function getEnumValues(schema: JsonObject) {
  return Array.isArray(schema.enum) ? schema.enum : [];
}

function getString(value: JsonValue | undefined) {
  return typeof value === "string" ? value : null;
}

function getNumber(value: JsonValue | undefined) {
  return typeof value === "number" ? value : null;
}

function getStringArray(value: JsonValue | undefined) {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function jsonEquals(left: JsonValue, right: JsonValue | undefined) {
  return right !== undefined && serializeJsonValue(left) === serializeJsonValue(right);
}

function serializeJsonValue(value: JsonValue | undefined) {
  return JSON.stringify(value);
}

function parseSerializedJsonValue(value: string) {
  return JSON.parse(value) as JsonValue;
}

function formatJsonForDisplay(value: JsonValue) {
  return typeof value === "string" ? value : JSON.stringify(value);
}

function isHttpUrl(value: string) {
  try {
    const url = new URL(value);
    return (url.protocol === "http:" || url.protocol === "https:") && Boolean(url.host);
  } catch {
    return false;
  }
}

function executionModeLabel(executionMode: AdapterMetadata["executionMode"]) {
  if (executionMode === "source_inventory") return "Quellenbestand";
  return "parametrisierte Suchanfrage";
}

function optionalText(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}
