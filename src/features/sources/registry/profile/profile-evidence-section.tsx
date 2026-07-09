import { Badge } from "@/components/reui/badge";
import {
  detectionEvidenceKindLabels,
  supportEvidenceKindLabels,
} from "@/features/sources/labels";
import type {
  DetectionEvidence,
  SupportEvidence,
} from "@/lib/api/sources";

type ProfileSupportEvidenceSectionProps = {
  evidence: SupportEvidence[];
};

export function ProfileSupportEvidenceSection({
  evidence,
}: ProfileSupportEvidenceSectionProps) {
  return (
    <section className="grid gap-2 rounded-lg border bg-muted/30 p-3">
      <div className="grid gap-1">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          Support-Evidenz
        </h3>
        <p className="text-xs text-muted-foreground">
          Deklarierte Support-Evidenz beschreibt die Einschätzung zum Profil.
          Die konkrete Nutzbarkeit wird über Source Live Checks geprüft.
        </p>
      </div>
      {evidence.length ? (
        <div className="flex flex-wrap gap-1.5">
          {evidence.map((item, index) => (
            <Badge
              key={`${item.kind}-${item.reference}-${index}`}
              variant="secondary"
              title={item.summary ?? item.reference}
            >
              {supportEvidenceKindLabels[item.kind]}
            </Badge>
          ))}
        </div>
      ) : (
        <span className="text-xs text-muted-foreground">
          Keine Support-Evidenz deklariert.
        </span>
      )}
    </section>
  );
}

type ProfileDetectionEvidenceSectionProps = {
  evidence: DetectionEvidence[];
};

export function ProfileDetectionEvidenceSection({
  evidence,
}: ProfileDetectionEvidenceSectionProps) {
  return (
    <section className="grid gap-2 rounded-lg border bg-muted/30 p-3">
      <div className="grid gap-1">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          Detection-Evidenz
        </h3>
        <p className="text-xs text-muted-foreground">
          Detection-Evidenz gehört zu detect.evidence und ist getrennt von
          Support-Evidenz. URL bleibt hier gültige Detection-Evidenz.
        </p>
      </div>
      {evidence.length ? (
        <div className="flex flex-wrap gap-1.5">
          {evidence.map((item, index) => (
            <Badge
              key={`${item.kind}-${item.message}-${index}`}
              variant="secondary"
              title={item.message}
            >
              {detectionEvidenceKindLabels[item.kind]}
            </Badge>
          ))}
        </div>
      ) : (
        <span className="text-xs text-muted-foreground">
          Keine Detection-Evidenz deklariert.
        </span>
      )}
    </section>
  );
}
