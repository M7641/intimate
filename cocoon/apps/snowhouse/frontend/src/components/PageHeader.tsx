import type { ReactNode } from "react";

/**
 * Consistent page header across the analytics pages: a title, an optional
 * (width-bounded) description, and a right-aligned slot for filters/actions.
 * Collapses to a stacked layout on narrow screens.
 *
 * Wrap each filter in {@link Field} so every page places its controls the same
 * way — the description is width-capped so it never crowds the filter row.
 */
export default function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <header className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
      <div className="min-w-0 space-y-1">
        <h1 className="text-lg font-semibold tracking-tight text-foreground">
          {title}
        </h1>
        {description && (
          <p className="max-w-2xl text-sm leading-relaxed text-muted-foreground">
            {description}
          </p>
        )}
      </div>
      {actions && (
        <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 lg:justify-end">
          {actions}
        </div>
      )}
    </header>
  );
}

/**
 * A labelled filter control for the header's actions slot. Keeps the label and
 * control on one baseline so a row of filters reads as a single, aligned toolbar
 * instead of ad-hoc pairs.
 */
export function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <label className="flex items-center gap-2 text-sm text-muted-foreground">
      <span className="whitespace-nowrap">{label}</span>
      {children}
    </label>
  );
}
