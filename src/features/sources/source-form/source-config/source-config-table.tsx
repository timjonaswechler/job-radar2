import { LockIcon, Trash2Icon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  configEntryDescription,
  schemaFieldType,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";

import {
  ConfigKeyControl,
  type ConfigKeyOption,
} from "./config-key-control";
import { ConfigValueControl } from "./config-value-control";

type SourceConfigTableProps = {
  entries: SourceConfigEntry[];
  keyOptions: ConfigKeyOption[];
  schemaMetadata: SchemaMetadata;
  disabled: boolean;
  portalContainer?: HTMLElement | null;
  onUpdate: (id: string, patch: Partial<SourceConfigEntry>) => void;
  onRemove: (id: string) => void;
};

export function SourceConfigTable({
  entries,
  keyOptions,
  schemaMetadata,
  disabled,
  portalContainer,
  onUpdate,
  onRemove,
}: SourceConfigTableProps) {
  return (
    <Table className="border-separate border-spacing-0 rounded-md border border-border text-xs [&_td]:border-r [&_td]:border-border [&_td:last-child]:border-r-0 [&_th]:border-r [&_th]:border-border [&_th:last-child]:border-r-0">
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead className="h-8 w-[32%] bg-muted/40 px-2">Key</TableHead>
          <TableHead className="h-8 bg-muted/40 px-2">Wert</TableHead>
          <TableHead className="h-8 w-28 bg-muted/40 px-2">Typ</TableHead>
          <TableHead className="h-8 w-12 bg-muted/40 px-1 text-right">
            <span className="sr-only">Aktionen</span>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody className="[&_tr:last-child]:border-b-0">
        {entries.map((entry, index) => {
          const propertySchema = schemaMetadata.properties.get(entry.key);
          const required = schemaMetadata.requiredKeys.has(entry.key);
          const locked = required && entry.locked === true;
          const fieldType = schemaFieldType(propertySchema);
          const description = entry.key
            ? configEntryDescription(entry.key, propertySchema, required)
            : "Freier Konfigurationswert.";

          return (
            <TableRow
              key={entry.id}
              className="hover:bg-transparent"
              title={description}
            >
              <TableCell className="whitespace-normal p-0 align-top transition-colors focus-within:bg-accent/30">
                <ConfigKeyControl
                  entry={entry}
                  index={index}
                  keyOptions={keyOptions}
                  locked={locked}
                  disabled={disabled}
                  portalContainer={portalContainer}
                  onChange={(key) => onUpdate(entry.id, { key })}
                />
              </TableCell>
              <TableCell className="whitespace-normal p-0 align-top transition-colors focus-within:bg-accent/30">
                <ConfigValueControl
                  entry={entry}
                  index={index}
                  propertySchema={propertySchema}
                  disabled={disabled}
                  portalContainer={portalContainer}
                  onChange={(value) => onUpdate(entry.id, { value })}
                />
              </TableCell>
              <TableCell className="whitespace-normal px-2 py-1.5 align-top text-muted-foreground">
                <div className="flex flex-col gap-1">
                  <span>{schemaFieldTypeLabel(fieldType)}</span>
                  {required ? <Badge variant="warning-light">Pflicht</Badge> : null}
                </div>
              </TableCell>
              <TableCell className="p-1 align-top text-right">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onRemove(entry.id)}
                  disabled={disabled || locked}
                  title={
                    locked
                      ? "Gespeicherter Pflichtwert kann nicht entfernt werden"
                      : "Wert entfernen"
                  }
                >
                  {locked ? (
                    <LockIcon aria-hidden="true" />
                  ) : (
                    <Trash2Icon aria-hidden="true" />
                  )}
                  <span className="sr-only">
                    {locked ? "Pflichtwert geschützt" : "Wert entfernen"}
                  </span>
                </Button>
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}

function schemaFieldTypeLabel(type: ReturnType<typeof schemaFieldType>) {
  if (type === "json") return "JSON";
  if (type === "number") return "Zahl";
  if (type === "boolean") return "Boolean";
  return "Text";
}
