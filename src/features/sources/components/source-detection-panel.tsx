import { AlertCircleIcon, CheckCircle2Icon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type {
  SourceDetectionMatch,
  SourceDetectionResult,
} from "@/lib/api/sources";

type SourceDetectionPanelProps = {
  result: SourceDetectionResult | null;
  onApplyMatch: (match: SourceDetectionMatch) => void;
};

export function SourceDetectionPanel({
  result,
  onApplyMatch,
}: SourceDetectionPanelProps) {
  if (!result) return null;

  if (result.status === "detected") {
    return (
      <Alert variant="success">
        <CheckCircle2Icon aria-hidden="true" />
        <AlertTitle>Profil erkannt</AlertTitle>
        <AlertDescription>
          <DetectionBadges result={result} />
          <EvidenceList evidence={result.evidence} warnings={result.warnings} />
        </AlertDescription>
      </Alert>
    );
  }

  if (result.status === "ambiguous") {
    return (
      <Alert variant="warning">
        <AlertCircleIcon aria-hidden="true" />
        <AlertTitle>Mehrere passende Profile gefunden</AlertTitle>
        <AlertDescription>
          <p>Wähle den passenden Vorschlag aus. Danach kannst du alle Felder weiter bearbeiten.</p>
          <div className="grid w-full gap-2">
            {result.matches.map((match) => (
              <Card key={`${match.profileKey}-${match.pathKey}-${match.key}`}>
                <CardHeader className="flex-row items-start justify-between gap-3">
                  <div className="grid gap-1">
                    <CardTitle>{match.name}</CardTitle>
                    <div className="flex flex-wrap gap-1">
                      <Badge variant="secondary">{match.profileName}</Badge>
                      <Badge variant="outline">{match.pathName ?? match.pathKey}</Badge>
                    </div>
                  </div>
                  <Button type="button" variant="outline" size="sm" onClick={() => onApplyMatch(match)}>
                    Übernehmen
                  </Button>
                </CardHeader>
                {match.evidence.length ? (
                  <CardContent>
                    <EvidenceList evidence={match.evidence} warnings={[]} />
                  </CardContent>
                ) : null}
              </Card>
            ))}
          </div>
          <EvidenceList evidence={result.evidence} warnings={result.warnings} />
        </AlertDescription>
      </Alert>
    );
  }

  if (result.status === "built_in_source") {
    return (
      <Alert variant="info">
        <CheckCircle2Icon aria-hidden="true" />
        <AlertTitle>Quelle ist bereits eingebaut</AlertTitle>
        <AlertDescription>
          <EvidenceList evidence={result.evidence} warnings={result.warnings} />
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <Alert variant="warning">
      <AlertCircleIcon aria-hidden="true" />
      <AlertTitle>Kein vorhandenes Profil erkannt</AlertTitle>
      <AlertDescription>
        <p>
          Du kannst dieselben Felder manuell ausfüllen. Der eingegebene Link wurde, falls möglich,
          als Konfigurationswert <code>startUrl</code> übernommen.
        </p>
        <EvidenceList evidence={result.evidence} warnings={result.warnings} />
      </AlertDescription>
    </Alert>
  );
}

function DetectionBadges({ result }: { result: SourceDetectionResult }) {
  return (
    <div className="flex flex-wrap gap-1">
      {result.profileName ? <Badge variant="secondary">{result.profileName}</Badge> : null}
      {result.pathName || result.pathKey ? (
        <Badge variant="outline">{result.pathName ?? result.pathKey}</Badge>
      ) : null}
      {result.key ? <Badge variant="primary-outline">{result.key}</Badge> : null}
    </div>
  );
}

function EvidenceList({ evidence, warnings }: { evidence: string[]; warnings: string[] }) {
  const visibleEvidence = evidence.slice(0, 3);
  const visibleWarnings = warnings.slice(0, 3);

  if (!visibleEvidence.length && !visibleWarnings.length) return null;

  return (
    <ul className="list-inside list-disc">
      {visibleEvidence.map((item) => (
        <li key={`evidence-${item}`}>{item}</li>
      ))}
      {visibleWarnings.map((item) => (
        <li key={`warning-${item}`}>{item}</li>
      ))}
    </ul>
  );
}
