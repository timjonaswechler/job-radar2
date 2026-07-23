import { AlertCircleIcon, CheckCircle2Icon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { supportLevelLabels } from "@/features/sources/labels";
import type {
  SourceProposal,
  SourceProposalDetectionResult,
  SourceProposalEvidence,
  StructuredDiagnostic,
} from "@/lib/api/sources";

type SourceDetectionPanelProps = {
  result: SourceProposalDetectionResult | null;
  applyDisabled?: boolean;
  onApplyProposal: (proposal: SourceProposal) => void;
};

export function SourceDetectionPanel({
  result,
  applyDisabled = false,
  onApplyProposal,
}: SourceDetectionPanelProps) {
  if (!result) return null;

  if (result.status === "matched" && result.proposals[0]) {
    return (
      <Alert variant="success">
        <CheckCircle2Icon aria-hidden="true" />
        <AlertTitle>Profil erkannt</AlertTitle>
        <AlertDescription>
          <DetectionBadges proposal={result.proposals[0]} />
          <ProposalDetails proposal={result.proposals[0]} />
          <EvidenceList evidence={result.proposals[0].evidence} diagnostics={result.diagnostics} />
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
            {result.proposals.map((proposal) => (
              <Card key={`${proposal.profileKey}-${proposal.recommendedAccessPathKey}`}>
                <CardHeader className="flex-row items-start justify-between gap-3">
                  <div className="grid gap-1">
                    <CardTitle>{proposal.nameCandidates[0] ?? proposal.profileName}</CardTitle>
                    <div className="flex flex-wrap gap-1">
                      <Badge variant="secondary">{proposal.profileName}</Badge>
                      <Badge variant="outline">{proposal.recommendedAccessPathName}</Badge>
                      <Badge variant="outline">{supportLevelLabels[proposal.supportLevel]}</Badge>
                    </div>
                  </div>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => onApplyProposal(proposal)}
                    disabled={applyDisabled}
                  >
                    Übernehmen
                  </Button>
                </CardHeader>
                <CardContent className="grid gap-2">
                  <ProposalDetails proposal={proposal} />
                  <EvidenceList evidence={proposal.evidence} diagnostics={[]} />
                </CardContent>
              </Card>
            ))}
          </div>
          <EvidenceList evidence={[]} diagnostics={result.diagnostics} />
        </AlertDescription>
      </Alert>
    );
  }

  const outcomeCopy = sourceDetectionOutcomeCopy(result);

  return (
    <Alert variant="warning">
      <AlertCircleIcon aria-hidden="true" />
      <AlertTitle>{outcomeCopy.title}</AlertTitle>
      <AlertDescription>
        <p>{outcomeCopy.description}</p>
        <EvidenceList evidence={unsupportedEvidence(result)} diagnostics={result.diagnostics} />
      </AlertDescription>
    </Alert>
  );
}

export function sourceDetectionOutcomeCopy(result: SourceProposalDetectionResult) {
  switch (result.status) {
    case "failed":
      return {
        title: "Profilerkennung fehlgeschlagen",
        description:
          "Die Prüfung konnte nicht abgeschlossen werden. Du kannst dieselben Felder manuell ausfüllen; es wurde kein Konfigurationswert automatisch übernommen.",
      };
    case "budget_exhausted":
      return {
        title: "Profilerkennung hat ihr Ausführungslimit erreicht",
        description:
          "Die begrenzte Prüfung konnte nicht vollständig abgeschlossen werden. Du kannst es erneut versuchen oder die Source manuell einrichten.",
      };
    case "cancelled":
      return {
        title: "Profilerkennung abgebrochen",
        description:
          "Die Prüfung wurde abgebrochen und hat keinen Vorschlag übernommen. Du kannst sie erneut starten oder die Source manuell einrichten.",
      };
    default:
      return {
        title: "Kein ausführbares Profil verfügbar",
        description:
          "Job Radar hat ein bekanntes, aber derzeit nicht unterstütztes Profil erkannt. Du kannst dieselben Felder manuell ausfüllen; der eingegebene Link wurde, falls möglich, als Konfigurationswert startUrl übernommen.",
      };
  }
}

function DetectionBadges({ proposal }: { proposal: SourceProposal }) {
  return (
    <div className="flex flex-wrap gap-1">
      <Badge variant="secondary">{proposal.profileName}</Badge>
      <Badge variant="outline">{proposal.recommendedAccessPathName}</Badge>
      <Badge variant="outline">{supportLevelLabels[proposal.supportLevel]}</Badge>
      {proposal.keyCandidates[0] ? <Badge variant="primary-outline">{proposal.keyCandidates[0]}</Badge> : null}
    </div>
  );
}

function ProposalDetails({ proposal }: { proposal: SourceProposal }) {
  const captures = Object.entries(proposal.captures);
  const configKeys = Object.keys(proposal.sourceConfig);

  return (
    <div className="grid gap-1 text-xs">
      <p>
        <span className="font-medium">Profil:</span> <code>{proposal.profileKey}</code>{" "}
        · <span className="font-medium">Access Path:</span>{" "}
        <code>{proposal.recommendedAccessPathKey}</code>
      </p>
      {configKeys.length ? (
        <p>
          <span className="font-medium">Source Config Vorschlag:</span>{" "}
          {configKeys.map((key) => (
            <code key={key} className="mr-1">{key}</code>
          ))}
        </p>
      ) : null}
      {captures.length ? (
        <p>
          <span className="font-medium">Captures:</span>{" "}
          {captures.map(([key, value]) => (
            <code key={key} className="mr-1">{key}={value}</code>
          ))}
        </p>
      ) : null}
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
