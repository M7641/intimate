// Chart.js draws to a <canvas> and cannot read CSS custom properties, so
// `var(--chart-1)` reaches it as an unparseable string and falls back to black.
// We resolve the theme tokens to their computed values here and recompute when
// the theme changes (see usage: gate a createEffect on the theme signal).

function cssVar(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  return value || fallback;
}

export interface ChartPalette {
  /** Up to 5 categorical series colours, in order. */
  series: string[];
  /** Subtle gridline colour. */
  grid: string;
  /** Axis tick / label colour. */
  ticks: string;
}

/**
 * Read the live chart palette from the active theme. Call inside a
 * createEffect/createMemo that depends on the theme accessor so it re-resolves
 * after the theme class is swapped on <html>.
 */
export function readChartPalette(): ChartPalette {
  return {
    series: [
      cssVar("--chart-1", "oklch(0.445 0.194 264.052)"),
      cssVar("--chart-2", "oklch(0.616 0.248 358.263)"),
      cssVar("--chart-3", "oklch(0.886 0.193 163.398)"),
      cssVar("--chart-4", "oklch(0.866 0.153 92.845)"),
      cssVar("--chart-5", "oklch(0.57 0.119 276.792)"),
    ],
    grid: cssVar("--border", "oklch(0.922 0 0)"),
    ticks: cssVar("--muted-foreground", "oklch(0.556 0 0)"),
  };
}
