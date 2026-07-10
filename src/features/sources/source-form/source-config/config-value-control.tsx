import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { SchemaGuidedValueEditor } from "@/features/sources/shared/schema-guided-value-editor";
import { schemaScalarOptions } from "@/features/sources/shared/schema-introspection";
import {
  jsonValueToInputValue,
  schemaFieldType,
  type JsonObject,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";

type ConfigValueControlProps = {
  entry: SourceConfigEntry;
  index: number;
  propertySchema: JsonObject | undefined;
  disabled: boolean;
  portalContainer?: HTMLElement | null;
  onChange: (value: string) => void;
};

export function ConfigValueControl({
  entry,
  index,
  propertySchema,
  disabled,
  portalContainer,
  onChange,
}: ConfigValueControlProps) {
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
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 shadow-none"
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
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 shadow-none"
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
        textareaClassName="min-h-16 rounded-none border-0 bg-transparent px-2 py-1.5 font-mono shadow-none"
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
      className="h-8 rounded-none border-0 bg-transparent shadow-none"
    />
  );
}

const booleanOptions = [
  { value: "true", label: "Ja / true" },
  { value: "false", label: "Nein / false" },
];

function schemaEnumOptions(schema: JsonObject | undefined) {
  return schemaScalarOptions(schema).map((option) => ({
    value: jsonValueToInputValue(option.value),
    label: option.label,
  }));
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
