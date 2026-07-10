import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
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
import { sourceStatusOptions } from "@/features/sources/status";
import type { SourceStatus } from "@/lib/api/sources";

type SourceNameFieldProps = {
  id: string;
  name: string;
  description: string;
  placeholder?: string;
  disabled: boolean;
  invalid: boolean;
  onChange: (name: string) => void;
};

export function SourceNameField({
  id,
  name,
  description,
  placeholder,
  disabled,
  invalid,
  onChange,
}: SourceNameFieldProps) {
  return (
    <Field data-invalid={invalid || undefined} data-disabled={disabled || undefined}>
      <FieldLabel htmlFor={id}>Name</FieldLabel>
      <Input
        id={id}
        value={name}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        aria-invalid={invalid || undefined}
        disabled={disabled}
      />
      <FieldDescription>{description}</FieldDescription>
      {invalid ? <FieldError>Name fehlt.</FieldError> : null}
    </Field>
  );
}

type SourceStatusFieldProps = {
  status: SourceStatus;
  description: string;
  disabled: boolean;
  selectPortalContainer?: HTMLElement | null;
  onChange: (status: SourceStatus) => void;
};

export function SourceStatusField({
  status,
  description,
  disabled,
  selectPortalContainer,
  onChange,
}: SourceStatusFieldProps) {
  return (
    <Field data-disabled={disabled || undefined}>
      <FieldLabel>Status</FieldLabel>
      <Select
        items={sourceStatusOptions}
        modal={false}
        value={status}
        onValueChange={(value) => {
          if (value) onChange(value as SourceStatus);
        }}
      >
        <SelectTrigger
          className="w-full"
          aria-label="Status wählen"
          disabled={disabled}
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
      <FieldDescription>{description}</FieldDescription>
    </Field>
  );
}
