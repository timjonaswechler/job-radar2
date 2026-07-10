import {
  Combobox,
  ComboboxCollection,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxGroup,
  ComboboxInput,
  ComboboxItem,
  ComboboxLabel,
  ComboboxList,
} from "@/components/ui/combobox";
import type { SourceConfigEntry } from "@/features/sources/shared/source-config-schema";

export type ConfigKeyOption = {
  key: string;
  label: string;
  description?: string;
  required: boolean;
};

type ConfigKeyControlProps = {
  entry: SourceConfigEntry;
  index: number;
  keyOptions: ConfigKeyOption[];
  locked: boolean;
  disabled: boolean;
  portalContainer?: HTMLElement | null;
  onChange: (key: string) => void;
};

export function ConfigKeyControl({
  entry,
  index,
  keyOptions,
  locked,
  disabled,
  portalContainer,
  onChange,
}: ConfigKeyControlProps) {
  const inputLocked = disabled || locked;
  const selectedOption =
    keyOptions.find((option) => option.key === entry.key) ?? null;
  const optionGroups = [{ value: "schema-keys", items: keyOptions }];

  return (
    <Combobox
      items={optionGroups}
      inputValue={entry.key}
      value={selectedOption}
      onInputValueChange={(value, eventDetails) => {
        // Closing a single-select combobox can reset its query. Only user edits
        // may replace a free-form Source Config key.
        if (eventDetails.reason === "input-change") onChange(value);
      }}
      onValueChange={(option) => {
        if (option) onChange(option.key);
      }}
      itemToStringLabel={(option) => option.key}
      itemToStringValue={(option) => option.key}
      isItemEqualToValue={(option, value) => option.key === value.key}
      autoHighlight
      disabled={inputLocked}
    >
      <ComboboxInput
        aria-label={`Key für Konfigurationswert ${index + 1}`}
        placeholder="Key"
        className="h-8 w-full rounded-none border-0 bg-transparent shadow-none"
        disabled={inputLocked}
        showTrigger={keyOptions.length > 0}
        data-vaul-no-drag=""
      />
      {keyOptions.length ? (
        <ComboboxContent
          className="min-w-64"
          portalContainer={portalContainer}
          data-vaul-no-drag=""
        >
          <ComboboxEmpty>
            Keine passenden Schema-Keys. Freie Eingabe ist möglich.
          </ComboboxEmpty>
          <ComboboxList>
            {(group) => (
              <ComboboxGroup key={group.value} items={group.items}>
                <ComboboxLabel>Bekannte Schema-Keys</ComboboxLabel>
                <ComboboxCollection>
                  {(option: ConfigKeyOption) => (
                    <ComboboxItem key={option.key} value={option}>
                      <div className="flex min-w-0 flex-col gap-0.5 pr-6">
                        <span className="truncate font-medium">{option.key}</span>
                        <span className="truncate text-muted-foreground">
                          {option.label}
                          {option.required ? " · Pflicht" : ""}
                        </span>
                        {option.description ? (
                          <span className="line-clamp-2 text-muted-foreground">
                            {option.description}
                          </span>
                        ) : null}
                      </div>
                    </ComboboxItem>
                  )}
                </ComboboxCollection>
              </ComboboxGroup>
            )}
          </ComboboxList>
        </ComboboxContent>
      ) : null}
    </Combobox>
  );
}
