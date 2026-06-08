import type { LucideIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

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
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          {Icon ? <Icon className="size-4 text-primary" aria-hidden="true" /> : null}
          {label}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-sm leading-6 text-muted-foreground">{description}</p>
      </CardContent>
    </Card>
  );
}
