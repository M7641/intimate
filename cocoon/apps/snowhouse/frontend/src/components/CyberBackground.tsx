import { useEffect, useRef } from "react";

import { useTheme } from "@/components/themeProvider";

// Pulsing neon-grid backdrop for the Cyber theme — a React port of the
// basecamp CyberBackdrop, kept in sync with that project's cyber mode.
//
// One full-viewport Canvas 2D layer behind all content. A faint cyan->magenta
// grid whose lines and nodes pulse in slow travelling waves: an undercurrent,
// not a focal point. Honors prefers-reduced-motion (one static frame), pauses
// when the tab is hidden, drifts subtly with the pointer, never intercepts
// pointer events. Canvas 2D (not WebGL) so it renders everywhere.

const GRID = 46; // cell size in px
const FRAME_MS = 1000 / 30; // the pulse is slow; 30fps is plenty
const SPEED = 0.0016; // wave travel speed (radians per ms)

// Cyan -> magenta lerp, returned as an rgba() string at the given alpha.
function neon(mix: number, alpha: number) {
  const r = Math.round(0 + 255 * mix);
  const g = Math.round(229 + (61 - 229) * mix);
  const b = Math.round(255 + (240 - 255) * mix);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export default function CyberBackground() {
  const { theme } = useTheme();
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (theme !== "cyber") return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d", { alpha: true });
    if (!ctx) return;

    const reduced = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;

    let cols = 0;
    let rows = 0;
    let raf = 0;
    let last = 0;
    const pointer = { x: 0.5, y: 0.5 }; // normalized, drives parallax

    function resize() {
      const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
      canvas!.width = Math.floor(window.innerWidth * dpr);
      canvas!.height = Math.floor(window.innerHeight * dpr);
      ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
      cols = Math.ceil(window.innerWidth / GRID) + 1;
      rows = Math.ceil(window.innerHeight / GRID) + 1;
    }

    // Per-axis wave: a column/row brightens as the travelling crest reaches it.
    const vWave = (c: number, t: number) => 0.5 + 0.5 * Math.sin(c * 0.5 - t);
    const hWave = (r: number, t: number) =>
      0.5 + 0.5 * Math.sin(r * 0.5 - t * 0.85 + 1.3);

    function drawFrame(now: number) {
      const t = now * SPEED;
      const w = window.innerWidth;
      const h = window.innerHeight;
      const px = (pointer.x - 0.5) * 12; // parallax drift, in px
      const py = (pointer.y - 0.5) * 12;

      ctx!.clearRect(0, 0, w, h);
      ctx!.lineWidth = 1;

      // Vertical lines — brightness pulses along the x axis.
      for (let c = 0; c < cols; c++) {
        const wv = vWave(c, t);
        const x = Math.round(c * GRID + px) + 0.5;
        ctx!.strokeStyle = neon(wv * 0.55, 0.05 + 0.13 * wv);
        ctx!.shadowBlur = wv > 0.7 ? 6 : 0;
        ctx!.shadowColor = "#00ffff";
        ctx!.beginPath();
        ctx!.moveTo(x, 0);
        ctx!.lineTo(x, h);
        ctx!.stroke();
      }

      // Horizontal lines — brightness pulses along the y axis.
      for (let r = 0; r < rows; r++) {
        const wv = hWave(r, t);
        const y = Math.round(r * GRID + py) + 0.5;
        ctx!.strokeStyle = neon(wv * 0.55, 0.05 + 0.13 * wv);
        ctx!.shadowBlur = wv > 0.7 ? 6 : 0;
        ctx!.shadowColor = "#00ffff";
        ctx!.beginPath();
        ctx!.moveTo(0, y);
        ctx!.lineTo(w, y);
        ctx!.stroke();
      }

      // Nodes glow only where both waves crest — a drifting constellation.
      ctx!.shadowColor = "#ff00ff";
      for (let c = 0; c < cols; c++) {
        const vw = vWave(c, t);
        if (vw < 0.6) continue;
        for (let r = 0; r < rows; r++) {
          const b = vw * hWave(r, t);
          if (b < 0.78) continue;
          ctx!.fillStyle = neon(b * 0.6, b * 0.7);
          ctx!.shadowBlur = 8 * b;
          ctx!.beginPath();
          ctx!.arc(c * GRID + px, r * GRID + py, 1.6, 0, Math.PI * 2);
          ctx!.fill();
        }
      }
      ctx!.shadowBlur = 0;
    }

    function staticFrame() {
      // reduced-motion: a single, evenly dim grid — no animation.
      const w = window.innerWidth;
      const h = window.innerHeight;
      ctx!.clearRect(0, 0, w, h);
      ctx!.lineWidth = 1;
      ctx!.strokeStyle = "rgba(0, 229, 255, 0.08)";
      ctx!.beginPath();
      for (let c = 0; c < cols; c++) {
        const x = Math.round(c * GRID) + 0.5;
        ctx!.moveTo(x, 0);
        ctx!.lineTo(x, h);
      }
      for (let r = 0; r < rows; r++) {
        const y = Math.round(r * GRID) + 0.5;
        ctx!.moveTo(0, y);
        ctx!.lineTo(w, y);
      }
      ctx!.stroke();
    }

    function loop(now: number) {
      raf = requestAnimationFrame(loop);
      if (now - last < FRAME_MS) return;
      last = now;
      drawFrame(now);
    }

    function start() {
      if (raf) return;
      if (reduced) {
        staticFrame();
        return; // no loop
      }
      last = 0;
      raf = requestAnimationFrame(loop);
    }

    function stop() {
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
    }

    function onVisibility() {
      if (document.hidden) stop();
      else start();
    }

    function onPointerMove(e: PointerEvent) {
      pointer.x = e.clientX / window.innerWidth;
      pointer.y = e.clientY / window.innerHeight;
    }

    let resizeTimer = 0;
    function onResize() {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        resize();
        if (reduced) staticFrame();
      }, 150);
    }

    resize();
    window.addEventListener("resize", onResize);
    window.addEventListener("pointermove", onPointerMove, { passive: true });
    document.addEventListener("visibilitychange", onVisibility);
    start();

    return () => {
      stop();
      window.clearTimeout(resizeTimer);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("pointermove", onPointerMove);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [theme]);

  if (theme !== "cyber") return null;

  return (
    <canvas
      ref={canvasRef}
      aria-hidden="true"
      style={{ opacity: 0.55 }}
      className="pointer-events-none fixed inset-0 -z-10 h-full w-full mix-blend-screen"
    />
  );
}
