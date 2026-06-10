import { useMemo } from "react";

import { AlertCircleIcon, Wand2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import type { JsonValue } from "@/lib/api/sources";

export type JsonParseResult =
  | { ok: true; value: JsonValue }
  | { ok: false; error: string };

export function parseJsonText(text: string): JsonParseResult {
  try {
    return { ok: true, value: JSON.parse(text) as JsonValue };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

type JsonFieldProps = {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  description?: string;
  placeholder?: string;
  rows?: number;
  disabled?: boolean;
};

export function JsonField({
  id,
  label,
  value,
  onChange,
  description,
  placeholder,
  rows = 8,
  disabled = false,
}: JsonFieldProps) {
  const parseResult = useMemo(() => parseJsonText(value), [value]);

  const handleFormat = () => {
    if (!parseResult.ok) return;
    onChange(JSON.stringify(parseResult.value, null, 2));
  };

  return (
    <div className="grid gap-1.5">
      <div className="flex items-center justify-between gap-2">
        <label className="text-xs font-medium" htmlFor={id}>
          {label}
        </label>
        <Button
          type="button"
          variant="outline"
          size="xs"
          onClick={handleFormat}
          disabled={disabled || !parseResult.ok}
        >
          <Wand2Icon className="size-3" aria-hidden="true" />
          Formatieren
        </Button>
      </div>
      {description ? (
        <p className="text-xs text-muted-foreground">{description}</p>
      ) : null}
      <Textarea
        id={id}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        rows={rows}
        disabled={disabled}
        aria-invalid={!parseResult.ok || undefined}
        className="min-h-32 font-mono"
      />
      {!parseResult.ok ? (
        <p className="flex items-center gap-1 text-xs text-destructive">
          <AlertCircleIcon className="size-3" aria-hidden="true" />
          Ungültiges JSON: {parseResult.error}
        </p>
      ) : (
        <p className="text-xs text-muted-foreground">Gültiges JSON.</p>
      )}
    </div>
  );
}
