import { useState } from "react";

import { ChevronDownIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { DetailRow } from "@/features/sources/registry/detail-row";
import { InlineDiagnostics } from "@/features/sources/registry/diagnostics/inline-diagnostics";
import { profileDslSchemaRefs } from "@/features/sources/shared/profile-dsl-schema-catalog";
import { OptionalSchemaValuePreview } from "@/features/sources/shared/schema-value-table";
import type { ProfileAccessPathDefinition } from "@/lib/api/sources";

type ProfileAccessPathDetailsProps = {
  accessPath: ProfileAccessPathDefinition;
};

export function ProfileAccessPathDetails({
  accessPath,
}: ProfileAccessPathDetailsProps) {
  const [open, setOpen] = useState(false);
  const capabilities = [
    accessPath.postingDiscovery ? "postingDiscovery" : null,
    accessPath.postingDetail ? "postingDetail" : null,
  ].filter(Boolean);

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border p-3"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="font-medium">{accessPath.name}</p>
          <p className="break-all font-mono text-xs text-muted-foreground">
            {accessPath.key}
          </p>
        </div>
        <div className="flex flex-wrap justify-end gap-1">
          {capabilities.map((capability) => (
            <Badge key={capability} variant="outline">
              {capability}
            </Badge>
          ))}
          <CollapsibleTrigger
            render={
              <Button
                type="button"
                variant="ghost"
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
            Details
          </CollapsibleTrigger>
        </div>
      </div>
      <CollapsibleContent className="mt-3 grid gap-3">
        <dl className="grid gap-3 sm:grid-cols-2">
          <DetailRow label="Pfad-Key" value={accessPath.key} mono />
          <DetailRow label="Pfad-Name" value={accessPath.name} />
          <DetailRow
            label="Fähigkeiten"
            value={capabilities.join(", ") || "—"}
          />
        </dl>
        {accessPath.description ? (
          <p className="text-xs text-muted-foreground">
            {accessPath.description}
          </p>
        ) : null}
        <div className="grid gap-2">
          {accessPath.diagnostics?.length ? (
            <InlineDiagnostics
              title="Diagnosen zu diesem Access Path"
              diagnostics={accessPath.diagnostics}
            />
          ) : null}
          <OptionalSchemaValuePreview
            title="sourceConfigSchema"
            description="Path-spezifisches Schema für Source Config. Search Request Kriterien gehören nicht hierher."
            value={accessPath.sourceConfigSchema}
          />
          <OptionalSchemaValuePreview
            title="knownIssues"
            description="Bekannte Einschränkungen dieses Access Path."
            value={accessPath.knownIssues}
          />
          <OptionalSchemaValuePreview
            title="postingDiscovery"
            description="Deklarative source-weite Posting Discovery."
            value={accessPath.postingDiscovery}
            schemaRef={profileDslSchemaRefs.postingDiscoveryStep}
          />
          <OptionalSchemaValuePreview
            title="postingDetail"
            description="Optionale lazy Posting Detail Extraktion für eine konkrete Posting-Quelle."
            value={accessPath.postingDetail}
            schemaRef={profileDslSchemaRefs.postingDetailStep}
          />
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
