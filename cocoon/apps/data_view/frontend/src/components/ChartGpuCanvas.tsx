import { useEffect, useRef, type CSSProperties } from "react";
import {
  createChart,
  type ChartGPUInstance,
  type ChartGPUOptions,
} from "@chartgpu/chartgpu";

interface ChartGpuCanvasProps {
  options: ChartGPUOptions;
  className?: string;
  style?: CSSProperties;
}

/**
 * Minimal, StrictMode-safe React wrapper over the ChartGPU core.
 *
 * We don't use `chartgpu-react` because its 0.1.4 mount guard is a single
 * shared flag: under StrictMode's mount→unmount→mount, the discarded first
 * `createChart` still resolves with the flag back to `true`, gets adopted, and
 * is then overwritten without disposal — leaving a second orphan canvas.
 *
 * The fix is a *per-effect* cancellation token: an async create that resolves
 * after its own effect was torn down disposes itself instead of being adopted.
 */
export default function ChartGpuCanvas({
  options,
  className,
  style,
}: ChartGpuCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ChartGPUInstance | null>(null);
  // Always-latest options, read by the async create without re-running it.
  const optionsRef = useRef(options);
  optionsRef.current = options;

  // Create exactly once per real mount.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let cancelled = false;
    createChart(container, optionsRef.current)
      .then((chart) => {
        if (cancelled) {
          chart.dispose(); // effect already torn down: discard, don't adopt
          return;
        }
        chartRef.current = chart;
      })
      .catch((e: unknown) => {
        if (!cancelled) console.error("ChartGPU init failed:", e);
      });

    return () => {
      cancelled = true;
      chartRef.current?.dispose();
      chartRef.current = null;
    };
  }, []);

  // Apply data/theme changes to the live chart (no GPU re-init).
  useEffect(() => {
    const chart = chartRef.current;
    if (chart && !chart.disposed) chart.setOption(options);
  }, [options]);

  // Keep the chart sized to its container.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(() => {
      const chart = chartRef.current;
      if (chart && !chart.disposed) chart.resize();
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  return <div ref={containerRef} className={className} style={style} />;
}
