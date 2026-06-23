import { useState } from "react";

import { ChevronDownIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";

type JsonPreviewProps = {
  title: string;
  value: unknown;
  description?: string;
  defaultOpen?: boolean;
};

export function JsonPreview({
  title,
  value,
  description,
  defaultOpen = false,
}: JsonPreviewProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border bg-muted/30 p-3"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="font-medium">{title}</p>
          {description ? (
            <p className="text-xs text-muted-foreground">{description}</p>
          ) : null}
        </div>
        <CollapsibleTrigger
          render={
            <Button
              type="button"
              variant="outline"
              size="xs"
              className="group"
            />
          }
        >
          <ChevronDownIcon
            data-icon="inline-start"
            className="transition-transform group-data-[state=open]:rotate-180"
            aria-hidden="true"
          />
          {open ? "Ausblenden" : "Anzeigen"}
        </CollapsibleTrigger>
      </div>
      <CollapsibleContent className="mt-3">
        <pre className="max-h-80 overflow-auto rounded-md bg-background p-3 font-mono text-xs">
          {JSON.stringify(value, null, 2)}
        </pre>
      </CollapsibleContent>
    </Collapsible>
  );
}

type OptionalJsonPreviewProps = Omit<JsonPreviewProps, "value"> & {
  value: unknown | null | undefined;
};

export function OptionalJsonPreview({
  value,
  ...props
}: OptionalJsonPreviewProps) {
  if (value === null || value === undefined) return null;
  return <JsonPreview value={value} {...props} />;
}
