import { SearchIcon, XIcon } from "lucide-react";

import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";

type RegistrySearchInputProps = {
  value: string;
  onChange: (value: string) => void;
  label: string;
  name: string;
  placeholder: string;
  clearLabel: string;
};

export function RegistrySearchInput({
  value,
  onChange,
  label,
  name,
  placeholder,
  clearLabel,
}: RegistrySearchInputProps) {
  return (
    <InputGroup className="w-56 bg-background">
      <InputGroupAddon align="inline-start">
        <SearchIcon aria-hidden="true" />
      </InputGroupAddon>
      <InputGroupInput
        aria-label={label}
        autoComplete="off"
        name={name}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {value.length > 0 ? (
        <InputGroupAddon align="inline-end">
          <InputGroupButton
            type="button"
            aria-label={clearLabel}
            title={clearLabel}
            size="icon-xs"
            onClick={() => onChange("")}
          >
            <XIcon aria-hidden="true" />
          </InputGroupButton>
        </InputGroupAddon>
      ) : null}
    </InputGroup>
  );
}
