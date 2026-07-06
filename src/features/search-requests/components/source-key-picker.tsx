import { useMemo, useState } from "react";

import { SearchIcon, XIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import { createSearchRequestSourceOptions } from "@/features/search-requests/model/source-options";
import type { RegistrySource, SourceKey } from "@/lib/api/sources";

type SourceKeyPickerProps = {
  sources: RegistrySource[];
  selectedSourceKeys: SourceKey[];
  disabled?: boolean;
  onChange: (sourceKeys: SourceKey[]) => void;
};

export function SourceKeyPicker({
  sources,
  selectedSourceKeys,
  disabled = false,
  onChange,
}: SourceKeyPickerProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const options = useMemo(
    () => createSearchRequestSourceOptions(sources, selectedSourceKeys),
    [selectedSourceKeys, sources],
  );
  const normalizedSearch = searchQuery.trim().toLocaleLowerCase("de");
  const filteredOptions = normalizedSearch
    ? options.filter((option) => option.searchText.includes(normalizedSearch))
    : options;

  const toggleSourceKey = (sourceKey: SourceKey, checked: boolean) => {
    if (checked) {
      onChange([...new Set([...selectedSourceKeys, sourceKey])]);
      return;
    }
    onChange(selectedSourceKeys.filter((selectedKey) => selectedKey !== sourceKey));
  };

  return (
    <FieldSet>
      <FieldLegend>Sources</FieldLegend>
      <FieldDescription>
        Auswahl basiert auf stabilen Source Keys aus der aktuellen Source Registry.
        Fehlende Keys aus bestehenden Search Requests bleiben sichtbar.
      </FieldDescription>
      <FieldGroup>
        <Field>
          <FieldLabel htmlFor="search-request-source-search">Sources suchen</FieldLabel>
          <InputGroup>
            <InputGroupAddon align="inline-start">
              <SearchIcon aria-hidden="true" />
            </InputGroupAddon>
            <InputGroupInput
              id="search-request-source-search"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Name, Key oder Status…"
              disabled={disabled}
            />
            {searchQuery ? (
              <InputGroupAddon align="inline-end">
                <InputGroupButton
                  type="button"
                  aria-label="Source-Suche leeren"
                  title="Source-Suche leeren"
                  size="icon-xs"
                  onClick={() => setSearchQuery("")}
                  disabled={disabled}
                >
                  <XIcon aria-hidden="true" />
                </InputGroupButton>
              </InputGroupAddon>
            ) : null}
          </InputGroup>
        </Field>

        <div className="grid max-h-64 gap-2 overflow-auto rounded-md border p-2">
          {filteredOptions.length ? (
            filteredOptions.map((option) => {
              const checked = selectedSourceKeys.includes(option.key);
              return (
                <div key={option.key} className="flex items-start gap-2 rounded-md p-1.5 hover:bg-muted/50">
                  <Checkbox
                    id={`search-request-source-${option.key}`}
                    checked={checked}
                    onCheckedChange={(nextChecked) =>
                      toggleSourceKey(option.key, nextChecked === true)
                    }
                    disabled={disabled}
                  />
                  <Label
                    htmlFor={`search-request-source-${option.key}`}
                    className="grid min-w-0 flex-1 gap-1 font-normal"
                  >
                    <span className="flex min-w-0 flex-wrap items-center gap-1.5">
                      <span className="truncate font-medium">{option.name}</span>
                      {option.missing ? (
                        <Badge variant="destructive-light">Fehlt</Badge>
                      ) : null}
                      {!option.canExecute && !option.missing ? (
                        <Badge variant="warning-light">Nicht ausführbar</Badge>
                      ) : null}
                    </span>
                    <span className="truncate font-mono text-muted-foreground">
                      {option.key} · {option.statusLabel} · {option.validationStateLabel}
                    </span>
                  </Label>
                </div>
              );
            })
          ) : (
            <p className="p-2 text-xs text-muted-foreground">Keine Sources gefunden.</p>
          )}
        </div>
      </FieldGroup>
    </FieldSet>
  );
}
