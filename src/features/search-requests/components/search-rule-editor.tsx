import { PlusIcon, Trash2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Field,
  FieldDescription,
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
import { searchRuleKindLabels } from "@/features/search-requests/labels";
import {
  emptySearchRuleDraft,
  type SearchRuleDraft,
} from "@/features/search-requests/model/search-request-form-model";
import type { SearchRuleKind } from "@/lib/api/search-requests";

const ruleKindOptions = Object.entries(searchRuleKindLabels).map(
  ([value, label]) => ({ value: value as SearchRuleKind, label }),
);

type SearchRuleEditorProps = {
  title: string;
  description: string;
  rules: SearchRuleDraft[];
  disabled?: boolean;
  onChange: (rules: SearchRuleDraft[]) => void;
};

export function SearchRuleEditor({
  title,
  description,
  rules,
  disabled = false,
  onChange,
}: SearchRuleEditorProps) {
  const updateRule = (index: number, nextRule: SearchRuleDraft) => {
    onChange(rules.map((rule, currentIndex) => (currentIndex === index ? nextRule : rule)));
  };

  const removeRule = (index: number) => {
    onChange(rules.filter((_, currentIndex) => currentIndex !== index));
  };

  return (
    <FieldSet>
      <FieldLegend>{title}</FieldLegend>
      <FieldDescription>{description}</FieldDescription>
      <FieldGroup>
        {rules.map((rule, index) => (
          <Field key={`${title}-${index}`}>
            <FieldLabel htmlFor={`${title}-${index}-value`}>
              Regel {index + 1}
            </FieldLabel>
            <div className="flex flex-col gap-2 sm:flex-row">
              <Select
                items={ruleKindOptions}
                modal={false}
                disabled={disabled}
                value={rule.kind}
                onValueChange={(value) => {
                  if (!value) return;
                  updateRule(index, { ...rule, kind: value as SearchRuleKind });
                }}
              >
                <SelectTrigger className="w-full sm:w-32" aria-label="Regeltyp wählen">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent alignItemWithTrigger={false}>
                  <SelectGroup>
                    {ruleKindOptions.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
              <Input
                id={`${title}-${index}-value`}
                value={rule.value}
                onChange={(event) => updateRule(index, { ...rule, value: event.target.value })}
                placeholder={rule.kind === "regex" ? "(?i)senior|staff" : "Senior Engineer"}
                disabled={disabled}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label="Regel entfernen"
                onClick={() => removeRule(index)}
                disabled={disabled}
              >
                <Trash2Icon aria-hidden="true" />
              </Button>
            </div>
          </Field>
        ))}
        <Button
          type="button"
          variant="outline"
          onClick={() => onChange([...rules, emptySearchRuleDraft()])}
          disabled={disabled}
        >
          <PlusIcon data-icon="inline-start" aria-hidden="true" />
          Regel hinzufügen
        </Button>
      </FieldGroup>
    </FieldSet>
  );
}
