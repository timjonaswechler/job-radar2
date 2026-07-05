import { DatabaseStatusCard } from "@/features/home/components/database-status-card";
import { InfrastructureCard } from "@/features/home/components/infrastructure-card";
import { infrastructureItems } from "@/lib/navigation";

export function HomeFeature() {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-2">
      <section className="rounded-lg border bg-card p-4 text-card-foreground shadow-xs">
        <header className="mb-4 flex flex-col gap-1.5">
          <h1 className="text-pretty text-base font-semibold">
            Neutraler Startpunkt
          </h1>
          <p className="max-w-3xl text-sm text-muted-foreground">
            Die technische Basis steht. Fachliche Konzepte bauen wir erst, wenn
            wir sie gemeinsam benennen.
          </p>
        </header>

        <div className="grid gap-4 md:grid-cols-3">
          {infrastructureItems.map((item) => (
            <InfrastructureCard key={item.label} {...item} />
          ))}
        </div>
      </section>

      <DatabaseStatusCard />
    </div>
  );
}
