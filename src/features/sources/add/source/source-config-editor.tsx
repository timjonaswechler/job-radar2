import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { createPortal } from "react-dom";

import { CheckIcon, ChevronDownIcon, LockIcon, PlusIcon, Trash2Icon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { createEntryId } from "@/features/sources/add/source/source-add-model";
import {
  configEntryDescription,
  jsonValueToInputValue,
  schemaDefaultValue,
  schemaFieldType,
  type JsonObject,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import { SchemaGuidedValueEditor } from "@/features/sources/shared/schema-guided-value-editor";
import { schemaScalarOptions } from "@/features/sources/shared/schema-introspection";

type SourceConfigEditorProps = {
  entries: SourceConfigEntry[];
  schemaMetadata: SchemaMetadata;
  disabled: boolean;
  configErrors: string[];
  showErrors: boolean;
  portalContainer?: HTMLElement | null;
  onChange: (entries: SourceConfigEntry[]) => void;
};

type ConfigKeyOption = {
  key: string;
  label: string;
  required: boolean;
};

export function SourceConfigEditor({
  entries,
  schemaMetadata,
  disabled,
  configErrors,
  showErrors,
  portalContainer,
  onChange,
}: SourceConfigEditorProps) {
  const knownKeys = [...schemaMetadata.properties.keys()];
  const keyOptions = knownKeys.map((key): ConfigKeyOption => {
    const schema = schemaMetadata.properties.get(key);
    return {
      key,
      label: schemaTitle(key, schema),
      required: schemaMetadata.requiredKeys.has(key),
    };
  });

  const addEntry = () => {
    const unusedKnownKey = knownKeys.find(
      (key) => !entries.some((entry) => entry.key === key),
    );
    const propertySchema = unusedKnownKey
      ? schemaMetadata.properties.get(unusedKnownKey)
      : undefined;

    onChange([
      ...entries,
      {
        id: createEntryId(),
        key: unusedKnownKey ?? "",
        value: jsonValueToInputValue(schemaDefaultValue(propertySchema)),
      },
    ]);
  };

  const updateEntry = (id: string, patch: Partial<SourceConfigEntry>) => {
    onChange(
      entries.map((entry) => (entry.id === id ? { ...entry, ...patch } : entry)),
    );
  };

  const removeEntry = (id: string) => {
    onChange(entries.filter((entry) => entry.id !== id));
  };

  return (
    <FieldSet>
      <FieldLegend>Quellenkonfiguration</FieldLegend>
      <FieldDescription>
        Werte werden als schema-geführte Key/Value-Tabelle gepflegt und beim
        Speichern in <code>sourceConfig</code> geschrieben.
      </FieldDescription>
      {keyOptions.length ? <SchemaKeyLegend options={keyOptions} /> : null}
      <FieldGroup>
        {entries.length ? (
          <Field>
            <FieldLabel className="sr-only">Konfigurationswerte</FieldLabel>
            <SourceConfigTable
              entries={entries}
              keyOptions={keyOptions}
              schemaMetadata={schemaMetadata}
              disabled={disabled}
              portalContainer={portalContainer}
              onUpdate={updateEntry}
              onRemove={removeEntry}
            />
            <FieldDescription>
              Pflichtwerte stammen aus dem effektiven Profil-/Access-Path-Schema.
              Bereits gespeicherte Pflichtwerte sind geschützt; neu hinzugefügte
              Pflichtwerte bleiben bis zum Speichern entfernbar.
            </FieldDescription>
          </Field>
        ) : (
          <SourceConfigEmptyState disabled={disabled} onAdd={addEntry} />
        )}

        {entries.length ? (
          <Button
            type="button"
            variant="outline"
            onClick={addEntry}
            disabled={disabled}
          >
            <PlusIcon data-icon="inline-start" aria-hidden="true" />
            Wert hinzufügen
          </Button>
        ) : null}

        {showErrors && configErrors.length ? (
          <FieldError>
            <ul className="list-inside list-disc">
              {configErrors.map((error) => (
                <li key={error}>{error}</li>
              ))}
            </ul>
          </FieldError>
        ) : null}
      </FieldGroup>
    </FieldSet>
  );
}

type SchemaKeyLegendProps = {
  options: ConfigKeyOption[];
};

function SchemaKeyLegend({ options }: SchemaKeyLegendProps) {
  return (
    <div className="flex flex-wrap gap-1">
      {options.map((option) => (
        <Badge
          key={option.key}
          variant={option.required ? "warning-light" : "outline"}
        >
          {option.key}
          {option.required ? " · Pflicht" : ""}
        </Badge>
      ))}
    </div>
  );
}

type SourceConfigTableProps = {
  entries: SourceConfigEntry[];
  keyOptions: ConfigKeyOption[];
  schemaMetadata: SchemaMetadata;
  disabled: boolean;
  portalContainer?: HTMLElement | null;
  onUpdate: (id: string, patch: Partial<SourceConfigEntry>) => void;
  onRemove: (id: string) => void;
};

function SourceConfigTable({
  entries,
  keyOptions,
  schemaMetadata,
  disabled,
  portalContainer,
  onUpdate,
  onRemove,
}: SourceConfigTableProps) {
  return (
    <Table className="border-separate border-spacing-0 rounded-md border border-border text-xs [&_td]:border-r [&_td]:border-border [&_td:last-child]:border-r-0 [&_th]:border-r [&_th]:border-border [&_th:last-child]:border-r-0">
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead className="h-8 w-[32%] bg-muted/40 px-2">Key</TableHead>
          <TableHead className="h-8 bg-muted/40 px-2">Wert</TableHead>
          <TableHead className="h-8 w-28 bg-muted/40 px-2">Typ</TableHead>
          <TableHead className="h-8 w-12 bg-muted/40 px-1 text-right">
            <span className="sr-only">Aktionen</span>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody className="[&_tr:last-child]:border-b-0">
        {entries.map((entry, index) => {
          const propertySchema = schemaMetadata.properties.get(entry.key);
          const required = schemaMetadata.requiredKeys.has(entry.key);
          const locked = required && entry.locked === true;
          const fieldType = schemaFieldType(propertySchema);
          const description = entry.key
            ? configEntryDescription(entry.key, propertySchema, required)
            : "Freier Konfigurationswert.";

          return (
            <TableRow
              key={entry.id}
              className="hover:bg-transparent"
              title={description}
            >
              <TableCell className="whitespace-normal p-0 align-top">
                <ConfigKeyCell
                  entry={entry}
                  index={index}
                  keyOptions={keyOptions}
                  locked={locked}
                  disabled={disabled}
                  portalContainer={portalContainer}
                  onChange={(key) => onUpdate(entry.id, { key })}
                />
              </TableCell>
              <TableCell className="whitespace-normal p-0 align-top">
                <ConfigValueCell
                  entry={entry}
                  index={index}
                  propertySchema={propertySchema}
                  disabled={disabled}
                  portalContainer={portalContainer}
                  onChange={(value) => onUpdate(entry.id, { value })}
                />
              </TableCell>
              <TableCell className="whitespace-normal px-2 py-1.5 align-top text-muted-foreground">
                <div className="flex flex-col gap-1">
                  <span>{schemaFieldTypeLabel(fieldType)}</span>
                  {required ? <Badge variant="warning-light">Pflicht</Badge> : null}
                </div>
              </TableCell>
              <TableCell className="p-1 align-top text-right">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onRemove(entry.id)}
                  disabled={disabled || locked}
                  title={
                    locked
                      ? "Gespeicherter Pflichtwert kann nicht entfernt werden"
                      : "Wert entfernen"
                  }
                >
                  {locked ? (
                    <LockIcon aria-hidden="true" />
                  ) : (
                    <Trash2Icon aria-hidden="true" />
                  )}
                  <span className="sr-only">
                    {locked ? "Pflichtwert geschützt" : "Wert entfernen"}
                  </span>
                </Button>
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}

type ConfigKeyCellProps = {
  entry: SourceConfigEntry;
  index: number;
  keyOptions: ConfigKeyOption[];
  locked: boolean;
  disabled: boolean;
  portalContainer?: HTMLElement | null;
  onChange: (key: string) => void;
};

function ConfigKeyCell({
  entry,
  index,
  keyOptions,
  locked,
  disabled,
  portalContainer,
  onChange,
}: ConfigKeyCellProps) {
  const [open, setOpen] = useState(false);
  const [popoverStyle, setPopoverStyle] = useState<CSSProperties | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const inputLocked = disabled || locked;
  const popoverRoot = portalContainer ?? document.body;

  const updatePopoverPosition = useCallback(() => {
    if (!inputRef.current || !popoverRoot) return;

    const inputRect = inputRef.current.getBoundingClientRect();
    const rootRect =
      popoverRoot === document.body
        ? { top: 0, left: 0, right: window.innerWidth }
        : popoverRoot.getBoundingClientRect();
    const minWidth = 256;
    const width = Math.max(inputRect.width, minWidth);
    const left = Math.min(
      Math.max(inputRect.left - rootRect.left, 8),
      Math.max(rootRect.right - rootRect.left - width - 8, 8),
    );

    setPopoverStyle({
      position: popoverRoot === document.body ? "fixed" : "absolute",
      top: inputRect.bottom - rootRect.top + 4,
      left,
      width,
    });
  }, [popoverRoot]);

  useLayoutEffect(() => {
    if (!open) return;
    updatePopoverPosition();
  }, [open, updatePopoverPosition]);

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (containerRef.current?.contains(target)) return;
      if (popoverRef.current?.contains(target)) return;
      setOpen(false);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("resize", updatePopoverPosition);
    document.addEventListener("scroll", updatePopoverPosition, true);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("resize", updatePopoverPosition);
      document.removeEventListener("scroll", updatePopoverPosition, true);
    };
  }, [open, updatePopoverPosition]);

  const chooseKey = (key: string) => {
    setOpen(false);
    onChange(key);
  };

  return (
    <div ref={containerRef} className="relative" data-vaul-no-drag="">
      <Input
        ref={inputRef}
        value={entry.key}
        onChange={(event) => onChange(event.target.value)}
        onFocus={() => {
          if (!inputLocked) setOpen(true);
        }}
        onClick={() => {
          if (!inputLocked) setOpen(true);
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") setOpen(false);
        }}
        aria-label={`Key für Konfigurationswert ${index + 1}`}
        placeholder="Key"
        className="h-8 rounded-none border-0 bg-transparent pr-8 shadow-none ring-0 focus-visible:ring-0"
        disabled={inputLocked}
        data-vaul-no-drag=""
      />
      {keyOptions.length && !inputLocked ? (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="absolute top-1/2 right-1 -translate-y-1/2"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => setOpen((current) => !current)}
          aria-label="Schema-Key-Auswahl öffnen"
          aria-expanded={open}
          data-vaul-no-drag=""
        >
          <ChevronDownIcon aria-hidden="true" />
        </Button>
      ) : null}
      {open && keyOptions.length && popoverStyle
        ? createPortal(
            <div
              ref={popoverRef}
              className="z-50 overflow-hidden rounded-lg bg-popover text-popover-foreground shadow-md ring-1 ring-foreground/10"
              style={popoverStyle}
              role="listbox"
              data-vaul-no-drag=""
            >
              <div className="px-2 py-1.5 text-xs text-muted-foreground">
                Bekannte Schema-Keys
              </div>
              <div className="h-px bg-border/50" />
              <div className="max-h-72 overflow-y-auto p-1">
                {keyOptions.map((option) => {
                  const selected = option.key === entry.key;

                  return (
                    <button
                      key={option.key}
                      type="button"
                      className="relative flex min-h-7 w-full cursor-default items-center rounded-md px-2 py-1 text-left text-xs/relaxed outline-hidden hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground"
                      role="option"
                      aria-selected={selected}
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={() => chooseKey(option.key)}
                      data-vaul-no-drag=""
                    >
                      <div className="flex min-w-0 flex-col gap-0.5 pr-6">
                        <span className="truncate font-medium">{option.key}</span>
                        <span className="truncate text-muted-foreground">
                          {option.label}
                          {option.required ? " · Pflicht" : ""}
                        </span>
                      </div>
                      {selected ? (
                        <CheckIcon
                          className="pointer-events-none absolute right-2 size-3.5"
                          aria-hidden="true"
                        />
                      ) : null}
                    </button>
                  );
                })}
              </div>
            </div>,
            popoverRoot,
          )
        : null}
    </div>
  );
}

type ConfigValueCellProps = {
  entry: SourceConfigEntry;
  index: number;
  propertySchema: JsonObject | undefined;
  disabled: boolean;
  portalContainer?: HTMLElement | null;
  onChange: (value: string) => void;
};

function ConfigValueCell({
  entry,
  index,
  propertySchema,
  disabled,
  portalContainer,
  onChange,
}: ConfigValueCellProps) {
  const enumOptions = schemaEnumOptions(propertySchema);
  const fieldType = schemaFieldType(propertySchema);
  const ariaLabel = `Wert für ${entry.key || `Konfigurationswert ${index + 1}`}`;

  if (enumOptions.length) {
    return (
      <Select
        items={enumOptions}
        modal={false}
        value={entry.value || null}
        onValueChange={(value) => {
          if (value !== null) onChange(value);
        }}
      >
        <SelectTrigger
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 shadow-none ring-0 focus:ring-0"
          aria-label={ariaLabel}
          disabled={disabled}
          data-vaul-no-drag=""
        >
          <SelectValue placeholder="Wert wählen" />
        </SelectTrigger>
        <SelectContent
          alignItemWithTrigger={false}
          portalContainer={portalContainer}
          data-vaul-no-drag=""
        >
          <SelectGroup>
            {enumOptions.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    );
  }

  if (fieldType === "boolean") {
    return (
      <Select
        items={booleanOptions}
        modal={false}
        value={normalizedBooleanValue(entry.value)}
        onValueChange={(value) => {
          if (value) onChange(value);
        }}
      >
        <SelectTrigger
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 shadow-none ring-0 focus:ring-0"
          aria-label={ariaLabel}
          disabled={disabled}
          data-vaul-no-drag=""
        >
          <SelectValue placeholder="Boolean wählen" />
        </SelectTrigger>
        <SelectContent
          alignItemWithTrigger={false}
          portalContainer={portalContainer}
          data-vaul-no-drag=""
        >
          <SelectGroup>
            {booleanOptions.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    );
  }

  if (fieldType === "json") {
    return (
      <SchemaGuidedValueEditor
        value={entry.value}
        onChange={onChange}
        ariaLabel={ariaLabel}
        schema={propertySchema}
        disabled={disabled}
        textareaClassName="min-h-16 rounded-none border-0 bg-transparent px-2 py-1.5 font-mono shadow-none ring-0 focus-visible:ring-0"
      />
    );
  }

  return (
    <Input
      value={entry.value}
      onChange={(event) => onChange(event.target.value)}
      placeholder="Wert"
      aria-label={ariaLabel}
      disabled={disabled}
      type={
        fieldType === "number"
          ? "number"
          : inputTypeForSchema(entry.key, propertySchema)
      }
      className="h-8 rounded-none border-0 bg-transparent shadow-none ring-0 focus-visible:ring-0"
    />
  );
}

type SourceConfigEmptyStateProps = {
  disabled: boolean;
  onAdd: () => void;
};

function SourceConfigEmptyState({
  disabled,
  onAdd,
}: SourceConfigEmptyStateProps) {
  return (
    <Empty className="rounded-md border border-dashed p-4">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <PlusIcon aria-hidden="true" />
        </EmptyMedia>
        <EmptyTitle>Noch keine Konfigurationswerte</EmptyTitle>
        <EmptyDescription>
          Füge Werte manuell hinzu oder nutze die Linkprüfung, um erkannte Werte
          zu übernehmen.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button
          type="button"
          variant="outline"
          onClick={onAdd}
          disabled={disabled}
        >
          <PlusIcon data-icon="inline-start" aria-hidden="true" />
          Wert hinzufügen
        </Button>
      </EmptyContent>
    </Empty>
  );
}

const booleanOptions = [
  { value: "true", label: "Ja / true" },
  { value: "false", label: "Nein / false" },
];

function schemaTitle(key: string, schema: JsonObject | undefined) {
  return typeof schema?.title === "string" ? schema.title : key;
}

function schemaEnumOptions(schema: JsonObject | undefined) {
  return schemaScalarOptions(schema).map((option) => ({
    value: jsonValueToInputValue(option.value),
    label: option.label,
  }));
}

function schemaFieldTypeLabel(type: ReturnType<typeof schemaFieldType>) {
  if (type === "json") return "JSON";
  if (type === "number") return "Zahl";
  if (type === "boolean") return "Boolean";
  return "Text";
}

function inputTypeForSchema(key: string, schema: JsonObject | undefined) {
  if (schema?.format === "uri" || /url$/i.test(key)) return "url";
  return "text";
}

function normalizedBooleanValue(value: string) {
  const normalized = value.trim().toLocaleLowerCase("de");
  if (["true", "1", "ja", "yes"].includes(normalized)) return "true";
  if (["false", "0", "nein", "no"].includes(normalized)) return "false";
  return null;
}
