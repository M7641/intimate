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
    <div className="rounded-lg border bg-card p-4 min-w-0">
      <p className="text-xs text-muted-foreground truncate" title={label}>
        {label}
      </p>
      <p className="text-xl font-semibold tracking-tight mt-1 tabular-nums">
        {value}
      </p>
      {sub && (
        <p className="text-xs text-muted-foreground mt-0.5 tabular-nums">
          {sub}
        </p>
      )}
    </div>
  );
}
