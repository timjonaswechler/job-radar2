type AppHeaderProps = {
  title: string;
  subtitle: string;
};

export function AppHeader({ title, subtitle }: AppHeaderProps) {
  return (
    <header className="border-b bg-background/85 px-6 py-5 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="grid gap-1">
        <p className="text-xs font-semibold uppercase tracking-[0.24em] text-primary">
          Local-first Desktop
        </p>
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        <p className="text-sm text-muted-foreground">{subtitle}</p>
      </div>
    </header>
  );
}
