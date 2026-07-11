import { useState } from "react";

import { PlusIcon, Trash2Icon } from "lucide-react";

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
  applySchemaGuidedArrayEdit,
  type SchemaGuidedArrayEdit,
} from "@/features/sources/schema-editor/schema-editor-edits";
import { SchemaGuidedEditError } from "@/features/sources/schema-editor/schema-guidance";
import type {
  SchemaGuidedEditableArrayRow,
  SchemaGuidedValueEditorModel,
} from "@/features/sources/schema-editor/schema-editor-model";
import {
  jsonValueToInputValue,
  SchemaValueControl,
} from "@/features/sources/schema-editor/schema-value-control";
import type {
  JsonObject,
  SchemaResolutionOptions,
} from "@/features/sources/shared/schema-introspection";

type ArrayEditorProps = {
  model: SchemaGuidedValueEditorModel;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions | undefined;
  disabled: boolean | undefined;
  onChange: (value: string) => void;
};

export function ArrayEditor({
  model,
  schema,
  schemaOptions,
  disabled,
  onChange,
}: ArrayEditorProps) {
  const [editError, setEditError] = useState<string | null>(null);

  if (!model.parseState.ok || !Array.isArray(model.parseState.value))
    return null;

  const applyEdit = (edit: SchemaGuidedArrayEdit) => {
    const result = applySchemaGuidedArrayEdit({
      rawText: model.rawText,
      schema,
      schemaOptions,
      edit,
    });
    if (result.ok) {
      setEditError(null);
      onChange(result.rawText);
      return;
    }
    setEditError(result.error);
  };

  return (
    <div className="flex flex-col gap-2">
      {model.editableArrayRows.length ? (
        <Table className={compactTableClassName()}>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead className="h-8 w-[20%] bg-muted/40 px-2">
                Index
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
            {model.editableArrayRows.map((row) => (
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
                        type: "set-item-value",
                        index: row.index,
                        rawValue,
                      })
                    }
                  />
                </TableCell>
                <TableCell className="whitespace-normal px-2 py-1.5 align-top">
                  <ArrayRowRule row={row} />
                </TableCell>
                <TableCell className="p-1 align-top text-right">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onClick={() =>
                      applyEdit({ type: "remove-item", index: row.index })
                    }
                    disabled={disabled}
                    title="Eintrag entfernen"
                  >
                    <Trash2Icon aria-hidden="true" />
                    <span className="sr-only">Eintrag entfernen</span>
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      ) : null}
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => applyEdit({ type: "add-item" })}
        disabled={disabled}
        className="w-fit"
      >
        <PlusIcon data-icon="inline-start" aria-hidden="true" />
        Eintrag hinzufügen
      </Button>
      {editError ? <SchemaGuidedEditError message={editError} /> : null}
    </div>
  );
}

function ArrayRowRule({ row }: { row: SchemaGuidedEditableArrayRow }) {
  if (!row.scalarOptions.length) {
    return <span className="text-muted-foreground">—</span>;
  }

  return (
    <div className="flex flex-wrap gap-1">
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
