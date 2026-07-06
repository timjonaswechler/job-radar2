import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

type LocationListEditorProps = {
  locationsText: string;
  radiusKmText: string;
  disabled?: boolean;
  onLocationsTextChange: (locationsText: string) => void;
  onRadiusKmTextChange: (radiusKmText: string) => void;
};

export function LocationListEditor({
  locationsText,
  radiusKmText,
  disabled = false,
  onLocationsTextChange,
  onRadiusKmTextChange,
}: LocationListEditorProps) {
  return (
    <FieldSet>
      <FieldLegend>Orte und Radius</FieldLegend>
      <FieldGroup>
        <Field>
          <FieldLabel htmlFor="search-request-locations">Orte</FieldLabel>
          <Textarea
            id="search-request-locations"
            value={locationsText}
            onChange={(event) => onLocationsTextChange(event.target.value)}
            placeholder={"Berlin\nRemote\nHamburg"}
            disabled={disabled}
          />
          <FieldDescription>
            Ein Ort pro Zeile oder kommagetrennt. Leere Einträge werden beim Speichern entfernt.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor="search-request-radius">Radius (km)</FieldLabel>
          <Input
            id="search-request-radius"
            value={radiusKmText}
            onChange={(event) => onRadiusKmTextChange(event.target.value)}
            placeholder="50"
            inputMode="numeric"
            disabled={disabled}
          />
          <FieldDescription>Leer lassen, wenn kein Radius gelten soll.</FieldDescription>
        </Field>
      </FieldGroup>
    </FieldSet>
  );
}
