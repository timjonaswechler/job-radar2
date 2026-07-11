import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type {
  JsonObject,
  SchemaFieldType,
  SchemaScalarOption,
} from "@/features/sources/shared/schema-introspection";
import type { JsonValue } from "@/lib/api/sources";

type SchemaValueControlProps = {
  value: JsonValue;
  valueKey: string;
  schema: JsonObject | undefined;
  fieldType: SchemaFieldType;
  scalarOptions: SchemaScalarOption[];
  disabled: boolean | undefined;
  onChange: (rawValue: string) => void;
};

export function SchemaValueControl({
  value,
  valueKey,
  schema,
  fieldType,
  scalarOptions,
  disabled,
  onChange,
}: SchemaValueControlProps) {
  if (scalarOptions.length) {
    return (
      <Select
        items={scalarOptions.map((option) => ({
          value: jsonValueToInputValue(option.value),
          label: option.label,
        }))}
        modal={false}
        value={jsonValueToInputValue(value) || null}
        onValueChange={(nextValue) => {
          if (nextValue !== null) onChange(nextValue);
        }}
      >
        <SelectTrigger
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 text-xs shadow-none ring-0 focus:ring-0"
          aria-label={`Wert für ${valueKey}`}
          disabled={disabled}
        >
          <SelectValue placeholder="Wert wählen" />
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false}>
          <SelectGroup>
            {scalarOptions.map((option) => (
              <SelectItem
                key={jsonValueToInputValue(option.value)}
                value={jsonValueToInputValue(option.value)}
              >
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
        value={typeof value === "boolean" ? String(value) : null}
        onValueChange={(nextValue) => {
          if (nextValue) onChange(nextValue);
        }}
      >
        <SelectTrigger
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 text-xs shadow-none ring-0 focus:ring-0"
          aria-label={`Wert für ${valueKey}`}
          disabled={disabled}
        >
          <SelectValue placeholder="Boolean wählen" />
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false}>
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
      <Textarea
        value={jsonValueToInputValue(value)}
        onChange={(event) => onChange(event.target.value)}
        aria-label={`Wert für ${valueKey}`}
        disabled={disabled}
        className="min-h-12 rounded-none border-0 bg-transparent px-2 py-1.5 font-mono text-xs shadow-none ring-0 focus-visible:ring-0"
      />
    );
  }

  return (
    <Input
      value={jsonValueToInputValue(value)}
      onChange={(event) => onChange(event.target.value)}
      aria-label={`Wert für ${valueKey}`}
      disabled={disabled}
      type={fieldType === "number" ? "number" : inputTypeForSchema(valueKey, schema)}
      className="h-8 rounded-none border-0 bg-transparent text-xs shadow-none ring-0 focus-visible:ring-0"
    />
  );
}

export function jsonValueToInputValue(value: JsonValue | undefined): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean")
    return String(value);
  return JSON.stringify(value);
}

function inputTypeForSchema(key: string, schema: JsonObject | undefined) {
  if (schema?.format === "uri" || /url$/i.test(key)) return "url";
  return "text";
}

const booleanOptions = [
  { value: "true", label: "Ja / true" },
  { value: "false", label: "Nein / false" },
];
