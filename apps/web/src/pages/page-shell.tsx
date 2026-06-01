import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"

export type PageCard = {
  title: string
  description: string
  status?: string
}

type PageShellProps = {
  eyebrow: string
  title: string
  description: string
  cards: PageCard[]
}

export function PageShell({
  eyebrow,
  title,
  description,
  cards,
}: PageShellProps) {
  return (
    <div className="flex flex-col gap-4">
      <section className="rounded-lg border bg-card p-6 text-card-foreground shadow-sm">
        <p className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
          {eyebrow}
        </p>
        <h1 className="mt-2 text-2xl font-semibold">{title}</h1>
        <p className="mt-2 max-w-3xl text-sm text-muted-foreground">
          {description}
        </p>
      </section>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        {cards.map((card) => (
          <Card key={card.title}>
            <CardHeader>
              <CardTitle>{card.title}</CardTitle>
              {card.status && (
                <Badge variant="outline" className="w-fit">
                  {card.status}
                </Badge>
              )}
            </CardHeader>
            <CardContent>
              <CardDescription>{card.description}</CardDescription>
            </CardContent>
          </Card>
        ))}
      </section>
    </div>
  )
}
