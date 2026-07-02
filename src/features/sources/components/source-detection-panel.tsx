import { AlertCircleIcon, CheckCircle2Icon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type {
  SourceProposal,
  SourceProposalDetectionResult,
  SourceProposalEvidence,
  StructuredDiagnostic,
} from "@/lib/api/sources";

type SourceDetectionPanelProps = {
  result: SourceProposalDetectionResult | null;
  onApplyProposal: (proposal: SourceProposal) => void;
};

export function SourceDetectionPanel({
  result,
  onApplyProposal,
}: SourceDetectionPanelProps) {
  if (!result) return null;

  if (result.status === "matched" && result.proposal) {
    return (
      <Alert variant="success">
        <CheckCircle2Icon aria-hidden="true" />
        <AlertTitle>Profil erkannt</AlertTitle>
        <AlertDescription>
          <DetectionBadges proposal={result.proposal} />
          <EvidenceList evidence={result.proposal.evidence} diagnostics={result.diagnostics} />
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
            {(result.proposals ?? []).map((proposal) => (
              <Card key={`${proposal.profileKey}-${proposal.recommendedAccessPathKey}`}>
                <CardHeader className="flex-row items-start justify-between gap-3">
                  <div className="grid gap-1">
                    <CardTitle>{proposal.nameCandidates[0] ?? proposal.profileName}</CardTitle>
                    <div className="flex flex-wrap gap-1">
                      <Badge variant="secondary">{proposal.profileName}</Badge>
                      <Badge variant="outline">{proposal.recommendedAccessPathName}</Badge>
                    </div>
                  </div>
                  <Button type="button" variant="outline" size="sm" onClick={() => onApplyProposal(proposal)}>
                    Übernehmen
                  </Button>
                </CardHeader>
                {proposal.evidence.length ? (
                  <CardContent>
                    <EvidenceList evidence={proposal.evidence} diagnostics={[]} />
                  </CardContent>
                ) : null}
              </Card>
            ))}
          </div>
          <EvidenceList evidence={[]} diagnostics={result.diagnostics} />
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
        <EvidenceList evidence={unsupportedEvidence(result)} diagnostics={result.diagnostics} />
      </AlertDescription>
    </Alert>
  );
}

function DetectionBadges({ proposal }: { proposal: SourceProposal }) {
  return (
    <div className="flex flex-wrap gap-1">
      <Badge variant="secondary">{proposal.profileName}</Badge>
      <Badge variant="outline">{proposal.recommendedAccessPathName}</Badge>
      {proposal.keyCandidates[0] ? <Badge variant="primary-outline">{proposal.keyCandidates[0]}</Badge> : null}
    </div>
  );
}

function EvidenceList({
  evidence,
  diagnostics,
}: {
  evidence: SourceProposalEvidence[];
  diagnostics: StructuredDiagnostic[];
}) {
  const visibleEvidence = evidence.slice(0, 3);
  const visibleDiagnostics = diagnostics.slice(0, 3);

  if (!visibleEvidence.length && !visibleDiagnostics.length) return null;

  return (
    <ul className="list-inside list-disc">
      {visibleEvidence.map((item) => (
        <li key={`evidence-${item.kind}-${item.message}`}>{item.message}</li>
      ))}
      {visibleDiagnostics.map((item) => (
        <li key={`diagnostic-${item.code}-${item.path}`}>{item.message}</li>
      ))}
    </ul>
  );
}

function unsupportedEvidence(result: SourceProposalDetectionResult) {
  return (result.unsupportedProfiles ?? []).flatMap((profile) => profile.evidence);
}
