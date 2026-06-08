import { Frame } from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import { DatabaseStatusCard } from "@/features/home/components/database-status-card";
import { InfrastructureCard } from "@/features/home/components/infrastructure-card";
import { infrastructureItems } from "@/lib/navigation";

export function HomeFeature() {
  return (
    <div className="grid gap-6">
      <Frame
        title="Neutraler Startpunkt"
        description="Die technische Basis steht. Fachliche Konzepte bauen wir erst, wenn wir sie gemeinsam benennen."
        action={<Button variant="outline">Bereit für den nächsten Slice</Button>}
      >
        <div className="grid gap-4 md:grid-cols-3">
          {infrastructureItems.map((item) => (
            <InfrastructureCard key={item.label} {...item} />
          ))}
        </div>
      </Frame>

      <DatabaseStatusCard />
    </div>
  );
}
