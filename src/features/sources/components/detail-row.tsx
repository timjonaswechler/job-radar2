type DetailRowProps = {
  label: string;
  value: string;
  mono?: boolean;
};

export function DetailRow({ label, value, mono = false }: DetailRowProps) {
  return (
    <div className="min-w-0">
      <dt className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </dt>
      <dd className={mono ? "break-all font-mono text-xs" : "break-words"}>
        {value}
      </dd>
    </div>
  );
}
