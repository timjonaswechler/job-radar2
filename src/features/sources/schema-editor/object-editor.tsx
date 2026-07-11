import { useState } from "react";

import { PlusIcon, Trash2Icon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  applySchemaGuidedObjectEdit,
  type SchemaGuidedObjectEdit,
} from "@/features/sources/schema-editor/schema-editor-edits";
import { SchemaGuidedEditError } from "@/features/sources/schema-editor/schema-guidance";
import type {
  SchemaGuidedEditableObjectRow,
  SchemaGuidedValueEditorModel,
} from "@/features/sources/schema-editor/schema-editor-model";
import {
  jsonValueToInputValue,
  SchemaValueControl,
} from "@/features/sources/schema-editor/schema-value-control";
import {
  isJsonObject,
  type JsonObject,
  type SchemaResolutionOptions,
} from "@/features/sources/shared/schema-introspection";

type ObjectEditorProps = {
  model: SchemaGuidedValueEditorModel;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions | undefined;
  disabled: boolean | undefined;
  onChange: (value: string) => void;
};

export function ObjectEditor({
  model,
  schema,
  schemaOptions,
  disabled,
  onChange,
}: ObjectEditorProps) {
  const [newKey, setNewKey] = useState("");
  const [editError, setEditError] = useState<string | null>(null);

  if (!model.parseState.ok || !isJsonObject(model.parseState.value))
    return null;

  const selectedKnownKey = model.availableObjectKeys.some(
    (option) => option.key === newKey,
  )
    ? newKey
    : null;
  const applyEdit = (edit: SchemaGuidedObjectEdit) => {
    const result = applySchemaGuidedObjectEdit({
      rawText: model.rawText,
      schema,
      schemaOptions,
      edit,
    });
    if (result.ok) {
      setEditError(null);
      onChange(result.rawText);
      return true;
    }
    setEditError(result.error);
    return false;
  };
  const addKey = () => {
    const key = newKey.trim();
    if (!key) return;
    if (applyEdit({ type: "add-property", key })) setNewKey("");
  };

  return (
    <div className="flex flex-col gap-2">
      {model.variantOptions.length > 1 ? (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs font-medium">Variante</span>
          <Select
            items={model.variantOptions.map((option) => ({
              value: String(option.index),
              label: option.label,
            }))}
            modal={false}
            value={
              model.activeVariantIndex === null
                ? null
                : String(model.activeVariantIndex)
            }
            onValueChange={(value) => {
              if (value !== null) {
                applyEdit({
                  type: "select-variant",
                  variantIndex: Number(value),
                });
              }
            }}
          >
            <SelectTrigger
              className="h-8 min-w-44 text-xs"
              aria-label="Schema-Variante auswählen"
              disabled={disabled}
            >
              <SelectValue placeholder="Variante wählen" />
            </SelectTrigger>
            <SelectContent alignItemWithTrigger={false}>
              <SelectGroup>
                {model.variantOptions.map((option) => (
                  <SelectItem key={option.index} value={String(option.index)}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>
      ) : null}

      {model.editableObjectRows.length ? (
        <Table className={compactTableClassName()}>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead className="h-8 w-[32%] bg-muted/40 px-2">
                Key
              </TableHead>
              <TableHead className="h-8 bg-muted/40 px-2">Wert</TableHead>
              <TableHead className="h-8 w-[22%] bg-muted/40 px-2">
                Regel
              </TableHead>
              <TableHead className="h-8 w-10 bg-muted/40 px-1 text-right">
                <span className="sr-only">Aktionen</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody className="[&_tr:last-child]:border-b-0">
            {model.editableObjectRows.map((row) => (
              <TableRow key={row.key} className="hover:bg-transparent">
                <TableCell className="whitespace-normal px-2 py-1.5 align-top font-mono">
                  {row.key}
                </TableCell>
                <TableCell className="p-0 align-top">
                  <SchemaValueControl
                    value={row.value}
                    valueKey={row.key}
                    schema={row.schema}
                    fieldType={row.fieldType}
                    scalarOptions={row.scalarOptions}
                    disabled={disabled}
                    onChange={(rawValue) =>
                      applyEdit({
                        type: "set-property-value",
                        key: row.key,
                        rawValue,
                      })
                    }
                  />
                </TableCell>
                <TableCell className="whitespace-normal px-2 py-1.5 align-top">
                  <ObjectRowRule row={row} />
                </TableCell>
                <TableCell className="p-1 align-top text-right">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onClick={() =>
                      applyEdit({ type: "remove-property", key: row.key })
                    }
                    disabled={disabled || row.required}
                    title={
                      row.required
                        ? "Pflicht-Key kann nicht entfernt werden"
                        : "Key entfernen"
                    }
                  >
                    <Trash2Icon aria-hidden="true" />
                    <span className="sr-only">Key entfernen</span>
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      ) : null}

      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={newKey}
          onChange={(event) => setNewKey(event.target.value)}
          placeholder="Object-Key"
          aria-label="Object-Key hinzufügen"
          disabled={disabled}
          className="h-8 min-w-36 flex-1 text-xs"
        />
        {model.availableObjectKeys.length ? (
          <Select
            items={model.availableObjectKeys.map((option) => ({
              value: option.key,
              label: option.key,
            }))}
            modal={false}
            value={selectedKnownKey}
            onValueChange={(value) => {
              if (value) setNewKey(value);
            }}
          >
            <SelectTrigger
              className="h-8 min-w-40 text-xs"
              aria-label="Schema-Key auswählen"
              disabled={disabled}
            >
              <SelectValue placeholder="Schema-Key" />
            </SelectTrigger>
            <SelectContent alignItemWithTrigger={false}>
              <SelectGroup>
                {model.availableObjectKeys.map((option) => (
                  <SelectItem key={option.key} value={option.key}>
                    {option.key}
                    {option.required ? " · Pflicht" : ""}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        ) : null}
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={addKey}
          disabled={disabled || !newKey.trim()}
        >
          <PlusIcon data-icon="inline-start" aria-hidden="true" />
          Key hinzufügen
        </Button>
      </div>
      {editError ? <SchemaGuidedEditError message={editError} /> : null}
    </div>
  );
}

function ObjectRowRule({ row }: { row: SchemaGuidedEditableObjectRow }) {
  if (!row.required && !row.unknown && !row.scalarOptions.length) {
    return <span className="text-muted-foreground">—</span>;
  }

  return (
    <div className="flex flex-wrap gap-1">
      {row.required ? <Badge variant="warning-light">Pflicht</Badge> : null}
      {row.unknown ? (
        <Badge variant="warning-light">not in schema</Badge>
      ) : null}
      {row.scalarOptions.map((option) => (
        <Badge key={jsonValueToInputValue(option.value)} variant="outline">
          {option.label}
        </Badge>
      ))}
    </div>
  );
}

function compactTableClassName() {
  return [
    "border-separate border-spacing-0 rounded-md border border-border text-xs",
    "[&_td]:border-r [&_td]:border-border [&_td:last-child]:border-r-0",
    "[&_th]:border-r [&_th]:border-border [&_th:last-child]:border-r-0",
  ].join(" ");
}
