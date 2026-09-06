export default function StatCard({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub?: string;
}) {
  return (
    <div className="min-w-0 rounded-xl border bg-card/60 p-4 backdrop-blur-sm transition-colors hover:border-border/80">
      <p
        className="truncate text-xs font-medium uppercase tracking-wide text-muted-foreground"
        title={label}
      >
        {label}
      </p>
      <p className="mt-1.5 text-2xl font-semibold tracking-tight tabular-nums">
        {value}
      </p>
      {sub && (
        <p
          className="mt-1 truncate text-xs text-muted-foreground tabular-nums"
          title={sub}
        >
          {sub}
        </p>
      )}
    </div>
  );
}
