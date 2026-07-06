import { useState } from "react";

import { ChevronDownIcon, FileTextIcon, PaperclipIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";

type AttachmentPlaceholder = {
  id: string;
  name: string;
  helper: string;
};

const attachmentPlaceholders = [
  {
    id: "cv",
    name: "Lebenslauf",
    helper: "Platzhalter",
  },
  {
    id: "certificates",
    name: "Zeugnisse",
    helper: "Noch nicht verknüpft",
  },
  {
    id: "cover-letter",
    name: "Anschreiben",
    helper: "später pro Anzeige",
  },
] satisfies AttachmentPlaceholder[];

export function PreviewAttachments() {
  const [open, setOpen] = useState(true);

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border bg-background p-3"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Bewerbungsunterlagen
          </p>
          <p className="text-xs text-muted-foreground">
            Platzhalter für schnelle Links zu Lebenslauf, Zeugnissen und
            Anschreiben.
          </p>
        </div>
        <CollapsibleTrigger
          render={
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="group p-0 font-normal text-muted-foreground hover:bg-transparent"
            />
          }
        >
          <PaperclipIcon data-icon="inline-start" aria-hidden="true" />
          {open ? "Ausblenden" : "Anzeigen"}
          <ChevronDownIcon
            data-icon="inline-end"
            className="transition-transform group-data-[state=open]:rotate-180"
            aria-hidden="true"
          />
        </CollapsibleTrigger>
      </div>

      <CollapsibleContent className="mt-3">
        <div className="flex flex-wrap gap-2">
          {attachmentPlaceholders.map((attachment) => (
            <Button
              key={attachment.id}
              type="button"
              size="xs"
              variant="secondary"
              disabled
            >
              <FileTextIcon data-icon="inline-start" aria-hidden="true" />
              <span className="font-normal">{attachment.name}</span>
              <span className="font-normal text-muted-foreground">
                {attachment.helper}
              </span>
            </Button>
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
