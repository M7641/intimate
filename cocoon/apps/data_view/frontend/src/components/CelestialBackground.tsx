import { useEffect, useRef } from "react";

import { useTheme } from "@/components/themeProvider";

// A subtle animated nebula rendered with raw WebGL2 (no library). It only
// mounts under the `celestial` theme and is purely decorative: pointer-events
// are off and it sits behind the app shell. Discipline over spectacle — this
// is a data tool, so the motion stays slow and the chroma low.
//
// Progressive enhancement: if WebGL2 is unavailable, the component renders
// nothing and the CSS static-nebula fallback (see celestial_effects.css)
// carries the look. Under prefers-reduced-motion we draw a single still frame.

const VERTEX_SHADER = `#version 300 es
in vec2 a_position;
void main() {
  gl_Position = vec4(a_position, 0.0, 1.0);
}`;

// Slow flowing nebula + sparse twinkling stars. Kept deliberately low-contrast
// so text and charts stay readable on top.
const FRAGMENT_SHADER = `#version 300 es
precision highp float;

out vec4 fragColor;

uniform vec2 u_resolution;
uniform float u_time;
uniform vec2 u_pointer;   // 0..1, eased cursor position
uniform float u_intensity; // master opacity of the nebula layer

// Hash / value-noise / fbm — standard cheap building blocks.
float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = hash(i);
  float b = hash(i + vec2(1.0, 0.0));
  float c = hash(i + vec2(0.0, 1.0));
  float d = hash(i + vec2(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
  float v = 0.0;
  float amp = 0.5;
  for (int i = 0; i < 5; i++) {
    v += amp * noise(p);
    p *= 2.0;
    amp *= 0.5;
  }
  return v;
}

vec2 hash2(vec2 p) {
  return vec2(hash(p), hash(p + 19.19));
}

// One depth layer of stars, evaluated in physical pixels so the cores stay
// crisp at any resolution. Each cell may hold one star with its own position,
// brightness, colour and twinkle phase. A 3x3 neighbour scan lets cores, halos
// and diffraction spikes spill across cell borders. The spikes argument gates
// the cross glints to the brightest stars only.
vec3 starLayer(
  vec2 px, float cell, float thresh,
  float core, float spikeLen, float spikes, float t
) {
  vec2 id = floor(px / cell);
  vec3 acc = vec3(0.0);
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 cid = id + vec2(float(i), float(j));
      if (hash(cid) < thresh) continue; // sparse: most cells are empty
      vec2 star = (cid + 0.15 + 0.7 * hash2(cid + 11.3)) * cell;
      vec2 d = px - star;
      float dist = length(d);
      float bright = 0.35 + 0.65 * hash(cid + 3.1);
      float phase = hash(cid + 7.7) * 6.2831;
      float tw = 0.7 + 0.3 * sin(t * 1.3 + phase); // gentle, never fully off
      float c = exp(-dist * dist / (core * core)); // crisp gaussian core
      float halo = exp(-dist / (core * 4.0)) * 0.16; // faint bloom
      // Four-point diffraction spikes: a thin bright cross.
      float sx = exp(-abs(d.y) * 1.1) * exp(-abs(d.x) / spikeLen);
      float sy = exp(-abs(d.x) * 1.1) * exp(-abs(d.y) / spikeLen);
      float spk = (sx + sy) * spikes * smoothstep(0.6, 0.95, bright);
      float v = (c + halo + spk) * bright * tw;
      // Mostly cool white; a few warm gold, a few icy blue.
      float w = hash(cid + 9.3);
      vec3 tint = vec3(0.92, 0.94, 1.0);
      if (w > 0.86) tint = vec3(1.0, 0.86, 0.66);
      else if (w < 0.16) tint = vec3(0.72, 0.85, 1.0);
      acc += tint * v;
    }
  }
  return acc;
}

void main() {
  vec2 uv = gl_FragCoord.xy / u_resolution.xy;
  float aspect = u_resolution.x / u_resolution.y;
  vec2 p = uv;
  p.x *= aspect;

  // Deep indigo base, matching the celestial theme's --background.
  vec3 col = vec3(0.055, 0.048, 0.105);

  // Drifting nebula. A second domain-warped fbm gives the soft cloud edges.
  float t = u_time * 0.015;
  vec2 q = p * 1.6 + vec2(t, t * 0.5);
  float warp = fbm(q + fbm(q * 0.5));
  float clouds = fbm(p * 1.2 + warp + vec2(-t * 0.6, t * 0.3));

  // Pointer pulls a faint glow toward the cursor — barely there.
  vec2 pc = u_pointer * vec2(aspect, 1.0);
  float pd = distance(p, pc);
  float pull = smoothstep(0.9, 0.0, pd) * 0.12;

  vec3 violet = vec3(0.42, 0.28, 0.62);
  vec3 blue = vec3(0.20, 0.32, 0.66);
  vec3 nebula = mix(blue, violet, smoothstep(0.3, 0.8, warp));

  float density = pow(smoothstep(0.30, 0.95, clouds), 1.5);
  col += nebula * density * (0.85 * u_intensity) + nebula * pull;

  // Vignette dims the gas at the edges — applied before the stars so points
  // stay crisp everywhere.
  float vig = smoothstep(1.25, 0.35, distance(uv, vec2(0.5)));
  col *= mix(0.78, 1.0, vig);

  // Starfield: three depth layers in physical pixels, each drifting at its own
  // slow rate for parallax. Tiny-and-many in front, sparse-and-bright behind.
  vec2 px = gl_FragCoord.xy;
  float drift = u_time * 1.6;
  vec3 sky = vec3(0.0);
  sky += starLayer(px + vec2(drift * 0.15, 0.0), 24.0, 0.88, 0.9, 6.0, 0.0, u_time) * 0.55;
  sky += starLayer(px + vec2(drift * 0.45, 0.0), 60.0, 0.90, 1.2, 9.0, 0.35, u_time) * 0.95;
  sky += starLayer(px + vec2(drift * 0.85, 0.0), 140.0, 0.93, 1.6, 16.0, 0.9, u_time) * 1.35;
  col += sky;

  fragColor = vec4(col, 1.0);
}`;

function compile(gl: WebGL2RenderingContext, type: number, src: string) {
  const shader = gl.createShader(type)!;
  gl.shaderSource(shader, src);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

export default function CelestialBackground() {
  const { theme } = useTheme();
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (theme !== "celestial") return;
    const canvas = canvasRef.current;
    if (!canvas) return;

    const gl = canvas.getContext("webgl2", {
      antialias: false,
      alpha: false,
      powerPreference: "low-power",
    });
    if (!gl) return; // CSS fallback carries the look.

    const vs = compile(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
    const fs = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
    if (!vs || !fs) return;

    const program = gl.createProgram()!;
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) return;
    gl.useProgram(program);

    // One fullscreen triangle.
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 3, -1, -1, 3]),
      gl.STATIC_DRAW,
    );
    const aPos = gl.getAttribLocation(program, "a_position");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

    const uResolution = gl.getUniformLocation(program, "u_resolution");
    const uTime = gl.getUniformLocation(program, "u_time");
    const uPointer = gl.getUniformLocation(program, "u_pointer");
    const uIntensity = gl.getUniformLocation(program, "u_intensity");

    const reduced = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;

    // Cap DPR: the nebula is soft, so extra pixels buy nothing but heat.
    const dpr = Math.min(window.devicePixelRatio || 1, 1.5);

    function resize() {
      const w = Math.floor(canvas!.clientWidth * dpr);
      const h = Math.floor(canvas!.clientHeight * dpr);
      if (canvas!.width !== w || canvas!.height !== h) {
        canvas!.width = w;
        canvas!.height = h;
      }
      gl!.viewport(0, 0, canvas!.width, canvas!.height);
    }

    // Eased pointer for the lazy glow-follow.
    let pointer = { x: 0.5, y: 0.5 };
    const target = { x: 0.5, y: 0.5 };
    function onPointerMove(e: PointerEvent) {
      target.x = e.clientX / window.innerWidth;
      target.y = 1.0 - e.clientY / window.innerHeight;
    }

    let raf = 0;
    let start = 0;

    function frame(now: number) {
      if (!start) start = now;
      const elapsed = (now - start) / 1000;
      resize();
      pointer.x += (target.x - pointer.x) * 0.04;
      pointer.y += (target.y - pointer.y) * 0.04;
      gl!.uniform2f(uResolution, canvas!.width, canvas!.height);
      gl!.uniform1f(uTime, elapsed);
      gl!.uniform2f(uPointer, pointer.x, pointer.y);
      gl!.uniform1f(uIntensity, 1.0);
      gl!.drawArrays(gl!.TRIANGLES, 0, 3);
      raf = requestAnimationFrame(frame);
    }

    function start_loop() {
      if (!raf) raf = requestAnimationFrame(frame);
    }
    function stop_loop() {
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
    }

    function onVisibility() {
      if (document.hidden) stop_loop();
      else if (!reduced) start_loop();
    }

    if (reduced) {
      // A single still frame: beautiful, but motionless.
      resize();
      gl.uniform2f(uResolution, canvas.width, canvas.height);
      gl.uniform1f(uTime, 12.0);
      gl.uniform2f(uPointer, 0.7, 0.7);
      gl.uniform1f(uIntensity, 1.0);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    } else {
      window.addEventListener("pointermove", onPointerMove, { passive: true });
      document.addEventListener("visibilitychange", onVisibility);
      start_loop();
    }

    return () => {
      stop_loop();
      window.removeEventListener("pointermove", onPointerMove);
      document.removeEventListener("visibilitychange", onVisibility);
      gl.deleteProgram(program);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      gl.deleteBuffer(buffer);
      // Deliberately NOT calling WEBGL_lose_context here: a canvas hands out a
      // single context, so losing it would poison the StrictMode re-mount,
      // which reuses the same canvas and the same (now dead) context. GC
      // reclaims it when the canvas leaves the DOM.
    };
  }, [theme]);

  if (theme !== "celestial") return null;

  return (
    <canvas
      ref={canvasRef}
      aria-hidden="true"
      className="pointer-events-none fixed inset-0 -z-10 h-full w-full"
    />
  );
}
