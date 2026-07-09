import { cn } from "@/lib/utils";

export type RegistryHealth = "valid" | "dependency_warning" | "invalid";
type RegistryStateTone = "ready" | "warning" | "invalid";

const registryStateDotClasses: Record<RegistryStateTone, string> = {
  ready: "bg-success",
  warning: "bg-warning",
  invalid: "bg-destructive",
};

type RegistryStateIndicatorProps = {
  health: RegistryHealth;
  diagnosticsCount: number;
};

export function RegistryStateIndicator({
  health,
  diagnosticsCount,
}: RegistryStateIndicatorProps) {
  const { label, tone } = registryStateIndicatorState(
    health,
    diagnosticsCount,
  );

  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      className="inline-flex size-4 shrink-0 items-center justify-center"
    >
      <span
        aria-hidden="true"
        className={cn(
          "size-2 rounded-full",
          registryStateDotClasses[tone],
        )}
      />
    </span>
  );
}

export function registryRowHealthClassName(health: RegistryHealth): string {
  switch (health) {
    case "invalid":
      return "bg-destructive/5 opacity-60 hover:bg-destructive/10";
    case "dependency_warning":
      return "bg-warning/5 hover:bg-warning/10";
    case "valid":
      return "";
  }
}

function registryStateIndicatorState(
  health: RegistryHealth,
  diagnosticsCount: number,
): { label: string; tone: RegistryStateTone } {
  switch (health) {
    case "invalid":
      return {
        label:
          diagnosticsCount > 0
            ? `Ungültig · ${diagnosticCountLabel(diagnosticsCount)} · Details öffnen`
            : "Ungültig",
        tone: "invalid",
      };
    case "dependency_warning":
      return {
        label: `Abhängigkeit unvollständig · ${diagnosticCountLabel(diagnosticsCount)} · Details öffnen`,
        tone: "warning",
      };
    case "valid":
      return { label: "Alles OK", tone: "ready" };
  }
}

function diagnosticCountLabel(count: number) {
  return `${count} Diagnose${count === 1 ? "" : "n"}`;
}
