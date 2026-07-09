import type { ReactNode } from "react";

import { Checkbox } from "@/components/ui/checkbox";
import { FieldLegend, FieldSet } from "@/components/ui/field";
import { Label } from "@/components/ui/label";

type RegistryFilterFieldsProps = {
  title: string;
  children: ReactNode;
};

export function RegistryFilterFields({
  title,
  children,
}: RegistryFilterFieldsProps) {
  return (
    <FieldSet className="gap-2">
      <FieldLegend
        variant="label"
        className="mb-0 text-xs leading-normal font-medium text-muted-foreground"
      >
        {title}
      </FieldLegend>
      <div className="grid gap-2">{children}</div>
    </FieldSet>
  );
}

type RegistryCheckboxFilterRowProps = {
  id: string;
  label: string;
  checked: boolean;
  count?: number;
  onCheckedChange: (checked: boolean) => void;
};

export function RegistryCheckboxFilterRow({
  id,
  label,
  checked,
  count,
  onCheckedChange,
}: RegistryCheckboxFilterRowProps) {
  return (
    <div className="flex items-center gap-2.5">
      <Checkbox
        id={id}
        checked={checked}
        onCheckedChange={(nextChecked) => onCheckedChange(nextChecked === true)}
      />
      <Label
        htmlFor={id}
        className="flex grow items-center justify-between gap-1.5 font-normal"
      >
        <span>{label}</span>
        {typeof count === "number" ? (
          <span className="text-muted-foreground">{count}</span>
        ) : null}
      </Label>
    </div>
  );
}
