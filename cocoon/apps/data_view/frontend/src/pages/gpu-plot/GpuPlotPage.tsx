import { useMemo, useState } from "react";
import { FaBraille, FaSyncAlt } from "react-icons/fa";
import {
  type ChartGPUOptions,
  type TooltipParams,
} from "@chartgpu/chartgpu";
import ChartGpuCanvas from "@/components/ChartGpuCanvas";
import { Button } from "@/components/ui/button";
import { useTheme } from "@/components/themeProvider";

const POINTS_PER_CLUSTER = 20_000;

// Three labelled Gaussian clusters. ChartGPU owns the palette/legend, so each
// becomes its own series — that's what makes the tooltip name the cluster and
// the legend toggle them independently.
const CLUSTERS = [
  { name: "Cluster A", cx: -2.2, cy: 1.4, spread: 0.9, color: "#4a85e8" },
  { name: "Cluster B", cx: 2.0, cy: -0.4, spread: 1.1, color: "#e85c7d" },
  { name: "Cluster C", cx: -0.3, cy: -2.3, spread: 0.7, color: "#2ebd8f" },
] as const;

/** Deterministic PRNG (mulberry32) so a given seed always yields the same cloud. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Build one scatter series per cluster. Points are emitted as parallel
 * Float32Arrays (ChartGPU's `XYArraysData`) — the GPU-friendly path that skips
 * an array-of-objects allocation for every one of the 60k points.
 */
function makeSeries(seed: number): ChartGPUOptions["series"] {
  const rand = mulberry32(seed);
  return CLUSTERS.map(({ name, cx, cy, spread, color }) => {
    const x = new Float32Array(POINTS_PER_CLUSTER);
    const y = new Float32Array(POINTS_PER_CLUSTER);
    for (let i = 0; i < POINTS_PER_CLUSTER; i++) {
      // Box–Muller: two uniforms -> one 2D Gaussian sample.
      const u1 = Math.max(rand(), 1e-7);
      const u2 = rand();
      const mag = spread * Math.sqrt(-2 * Math.log(u1));
      x[i] = cx + mag * Math.cos(2 * Math.PI * u2);
      y[i] = cy + mag * Math.sin(2 * Math.PI * u2);
    }
    return { type: "scatter", name, color, symbolSize: 4, data: { x, y } };
  });
}

/** WebGPU is required; ChartGPU throws on create without it, so guard up front. */
function hasWebGpu(): boolean {
  return typeof navigator !== "undefined" && "gpu" in navigator;
}

export default function GpuPlotPage() {
  const { theme } = useTheme();
  const [seed, setSeed] = useState(1);

  // ChartGPU only knows 'light' | 'dark'; fold the app's extra themes onto dark.
  const chartTheme = theme === "light" ? "light" : "dark";

  const options = useMemo<ChartGPUOptions>(
    () => ({
      series: makeSeries(seed),
      theme: chartTheme,
      xAxis: { type: "value", name: "x" },
      yAxis: { type: "value", name: "y" },
      legend: { show: true, position: "top" },
      tooltip: {
        show: true,
        trigger: "item",
        formatter: (p: TooltipParams) =>
          `${p.seriesName} — x ${p.value[0].toFixed(2)}, y ${p.value[1].toFixed(2)}`,
      },
    }),
    [seed, chartTheme],
  );

  return (
    <div className="max-w-4xl space-y-6">
      <header className="flex items-start justify-between gap-4">
        <div>
          <h1 className="flex items-center gap-2 text-xl font-semibold">
            <FaBraille className="text-muted-foreground" />
            GPU Scatter
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {(POINTS_PER_CLUSTER * CLUSTERS.length).toLocaleString()} points drawn
            with WebGPU via ChartGPU. Hover a point for its values; the legend
            toggles clusters and you can scroll to zoom.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setSeed((s) => s + 1)}
          className="shrink-0"
        >
          <FaSyncAlt className="mr-2 h-3 w-3" />
          Regenerate
        </Button>
      </header>

      <div className="rounded-lg border bg-card p-4">
        {hasWebGpu() ? (
          <ChartGpuCanvas
            options={options}
            style={{ width: "100%", height: "480px" }}
          />
        ) : (
          <div className="flex h-[480px] items-center justify-center rounded-md border border-dashed bg-muted/30 p-6 text-center text-sm text-muted-foreground">
            WebGPU is not supported in this browser. Try Chrome/Edge 113+ or
            Safari 18+.
          </div>
        )}
      </div>
    </div>
  );
}
