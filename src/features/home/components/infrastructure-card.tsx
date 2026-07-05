import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

type InfrastructureCardProps = {
  label: string;
  description: string;
  icon?: LucideIcon;
};

export function InfrastructureCard({
  label,
  description,
  icon: Icon,
}: InfrastructureCardProps) {
  return (
    <article className="rounded-lg border bg-background p-4 shadow-xs">
      <div className="flex min-w-0 items-start gap-2">
        {Icon ? (
          <span
            aria-hidden="true"
            className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary"
          >
            <Icon aria-hidden="true" />
          </span>
        ) : null}
        <div className={cn("min-w-0", Icon ? "pt-0.5" : null)}>
          <h3 className="truncate text-sm font-medium">{label}</h3>
          <p className="mt-2 text-sm leading-6 text-muted-foreground">
            {description}
          </p>
        </div>
      </div>
    </article>
  );
}
