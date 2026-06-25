import {
  Frame,
  FrameDescription,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import { DatabaseStatusCard } from "@/features/home/components/database-status-card";
import { InfrastructureCard } from "@/features/home/components/infrastructure-card";
import { infrastructureItems } from "@/lib/navigation";

export function HomeFeature() {
  return (
    <div className="grid gap-4 p-2">
      <Frame>
        <FramePanel>
          <FrameHeader className="gap-4 sm:flex-row sm:items-start sm:justify-between">
            <div className="grid gap-1.5">
              <FrameTitle>Neutraler Startpunkt</FrameTitle>
              <FrameDescription>
                Die technische Basis steht. Fachliche Konzepte bauen wir erst,
                wenn wir sie gemeinsam benennen.
              </FrameDescription>
            </div>
            <Button variant="outline">Bereit für den nächsten Slice</Button>
          </FrameHeader>

          <div className="grid gap-4 md:grid-cols-3">
            {infrastructureItems.map((item) => (
              <InfrastructureCard key={item.label} {...item} />
            ))}
          </div>
        </FramePanel>
      </Frame>

      <DatabaseStatusCard />
    </div>
  );
}
