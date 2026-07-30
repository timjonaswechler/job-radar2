import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";

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
