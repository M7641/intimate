# Best-in-Class Remote Desktop Streaming — Open-Source Stack

## 1. Scope and the one honest caveat

"Open source" here means **the streaming server, transport, compositor, and desktop are all
OSS**. There is exactly one unavoidable proprietary dependency to be upfront about:

> **The GPU driver.** NVIDIA's `nvenc`/driver is proprietary (closed-source kernel module +
> userspace). The _streaming software_ that drives it (GStreamer, Selkies, Sunshine) is fully
> open — but the encoder it calls runs through NVIDIA's blob.

If you want a **100% open driver stack too**, use **VA-API** on an **Intel** (Arc / Flex) or
**AMD** GPU instead of NVENC on NVIDIA. The Mesa + VA-API path is entirely open source. The
trade-off: NVIDIA + NVENC has the most mature tooling and the widest cloud availability
(every AWS `g5`/`g6` is NVIDIA), so most real-world open-source GPU streaming today runs OSS
software on top of the NVIDIA blob.

| Hardware         | Encoder                | Driver licence        | Cloud availability           |
| ---------------- | ---------------------- | --------------------- | ---------------------------- |
| NVIDIA L4 / A10G | NVENC (H.264/HEVC/AV1) | Proprietary blob      | Best (AWS g5/g6, GCP, Azure) |
| Intel Arc / Flex | VA-API (QSV)           | **Fully open (Mesa)** | Limited (some clouds)        |
| AMD              | VA-API (VCN)           | **Fully open (Mesa)** | Limited                      |

---

## 2. The open-source landscape

| Project                                 | Licence    | Transport          | Encoding                      | Client                        | Best for                                 |
| --------------------------------------- | ---------- | ------------------ | ----------------------------- | ----------------------------- | ---------------------------------------- |
| **Selkies** (`selkies-project/selkies`) | Apache-2.0 | **WebRTC**         | NVENC / VA-API, incl. **AV1** | **Browser**                   | Containerised GPU desktop in the browser |
| **Sunshine** (`LizardByte/Sunshine`)    | GPLv3      | RTP (GameStream)   | NVENC / VA-API / AMF / QSV    | **Moonlight** (native, GPLv3) | Lowest latency, highest fps              |
| **Neko** (`m1k1o/neko`)                 | Apache-2.0 | WebRTC             | CPU or GPU (GStreamer)        | **Browser**                   | Multi-user / shared sessions             |
| **Xpra**                                | GPLv2      | Custom + **HTML5** | NVENC / x264 / vp9 / AV1      | Browser or native             | App-forwarding ("rootless") + desktop    |
| **wayvnc** + sway                       | GPLv2      | VNC/RFB            | CPU (frame-based)             | Browser (noVNC)               | Wayland tiling, GPU-less is fine         |
| **KasmVNC** (current)                   | OSS        | VNC/RFB            | CPU JPEG/WebP                 | Browser (noVNC)               | What we run today                        |

---

## 3. The pick: **Selkies** — the open-source equivalent of Amazon DCV

If the priority is **browser-delivered, containerised, beautiful, smooth**, Selkies is the
clear #1 open-source choice. It is, in effect, the OSS counterpart to Amazon DCV.

**Why it wins:**

- **WebRTC transport** — UDP, adaptive bitrate, designed for real-time video. This is the
  single biggest framerate upgrade over our VNC/RFB pipeline.
- **Hardware encoding** via GStreamer — `nvh264enc` / `nvav1enc` (NVENC) or VA-API. **AV1**
  is supported on the L4, giving the best quality-per-bitrate.
- **Browser-native** — no client install; an HTML5 frontend connects over WebRTC. Same UX
  model as our current noVNC button, just dramatically faster.
- **Container- and Kubernetes-native** — it was built for exactly our deployment shape
  (headless GPU pod, HTTP/WebSocket signalling on a port). This is what powers GPU desktops
  in Google Colab and the GPU variants of linuxserver.io's "webtop" images.
- **Audio, clipboard, file transfer, gamepad** built in.

**Architecture (how it fits our model):**

```
GPU pod (AWS g6 / L4)
├── Headless X11 or Wayland session  ─ KDE Plasma 6 / Hyprland (GPU-rendered)
├── GStreamer pipeline               ─ capture → NVENC (H.264/AV1) → WebRTC
└── Selkies signalling server (HTTP/WebSocket on TARGETPORT)
        │  WebRTC (UDP)
        ▼
   Browser (HTML5 client) ── no install
```

This maps onto the Nimbus contract the same way KasmVNC does today: serve HTTP on the target
port, auth via injected env, persistent `$HOME` volume. The substitution is **KasmVNC →
Selkies**, plus a **GPU node**.

---

## 4. Runner-up for raw framerate: **Sunshine + Moonlight**

Both fully open source (GPLv3). This is **cloud-gaming-grade**: the lowest latency and
highest framerate of any open-source option — up to 4K/120fps, HDR.

- **Sunshine** = the host (encodes via NVENC/VA-API/AMF/QSV, has a web config UI).
- **Moonlight** = the client (open source, every platform).

**Caveat for our use case:** the best Moonlight experience is a **native client**, not a
browser tab. There are web/embedded Moonlight efforts but they're less mature than the native
apps. So Sunshine+Moonlight wins on pure performance but loses on the "open a URL and it's
just there" UX that Selkies gives. Pick this if absolute latency beats zero-install.

---

## 5. Honourable mentions

- **Neko** (Apache-2.0) — WebRTC, browser-native, **multi-user**. Originally for shared
  browsing / watch-together, but runs full XFCE/KDE desktops and supports GPU encoding via
  GStreamer. Reach for it if collaborative/shared sessions matter more than peak single-user
  framerate.
- **Xpra** (GPLv2) — "screen for X": forward individual apps _or_ a full desktop, HTML5
  client, supports NVENC/AV1. Great when you want **rootless app forwarding** (stream one
  app, not a whole desktop) alongside desktop mode.
- **wayvnc + sway** (GPLv2) — the pragmatic **Wayland** path that works **without a GPU**.
  Software-renders well and pairs with sway's tiling. It won't hit WebRTC framerates (still
  VNC/RFB), but it's the cleanest open-source Wayland-tiling option if you ever stay CPU-only.

---

## 6. Recommended fully-open-source stack, component by component

| Layer                            | Component                                                       | Licence                |
| -------------------------------- | --------------------------------------------------------------- | ---------------------- |
| **Hardware**                     | AWS `g6` (NVIDIA L4) — or Intel Arc/Flex for a 100% open driver | —                      |
| **Driver / encoder**             | NVENC (NVIDIA blob) — or **VA-API + Mesa** for fully open       | Proprietary / **Open** |
| **Capture + encode + transport** | **Selkies** (GStreamer → NVENC/VA-API → WebRTC)                 | Apache-2.0             |
| **Compositor / DE**              | **KDE Plasma 6 (Wayland)** — or **Hyprland** for max eye-candy  | GPL/LGPL/BSD           |
| **Client**                       | Selkies HTML5 frontend (browser, no install)                    | Apache-2.0             |
| **Base image**                   | Ubuntu 24.04 (what we already migrated to)                      | —                      |

> For a **zero-proprietary-component** build: swap the L4 for an **Intel Arc/Flex** GPU and
> use **VA-API**. Everything else stays identical. The cost is narrower cloud availability.

---

## 7. Why this is a clean migration from what we have today

We are already most of the way there:

- ✅ **Ubuntu 24.04 base** — Selkies targets it directly.
- ✅ **Containerised, single HTTP port, env-based auth, persistent `$HOME`** — Selkies fits
  the same Nimbus contract as KasmVNC.
- ✅ **The desktop seeding / skel pattern** we built carries over unchanged.
- 🔄 **Swap KasmVNC → Selkies** in the entrypoint and the desktop block.
- 🔄 **Schedule onto a GPU node** (the only infra change that actually matters).
- 🔄 **Upgrade XFCE → KDE Plasma 6 / Hyprland** — now viable because the GPU does the
  rendering (recall KDE/Hyprland failed _only_ because we had no GPU).

The conceptual change is small; the hardware change (GPU node) is what unlocks everything.

---

## 8. Recommendation

> **AWS `g6` (NVIDIA L4) + Selkies (GStreamer/WebRTC, NVENC/AV1) + KDE Plasma 6 (Wayland),
> on our existing Ubuntu 24.04 container.**

This is the best-in-class **open-source** stack: browser-native, hardware-accelerated, AV1,
Kubernetes-friendly, and a small delta from our current image. If you ever need a fully open
_driver_ stack too, switch the GPU to Intel Arc/Flex + VA-API with no other changes.

Use **Sunshine + Moonlight** instead only if you'll accept a native client in exchange for
the absolute lowest latency.

---

## 9. Next steps

1. **Prototype Selkies** in a throwaway container on a `g6` to validate
   GStreamer → NVENC → WebRTC end-to-end.
2. **Swap the entrypoint** desktop launch from KasmVNC to Selkies behind the same target
   port, keeping the skel-seeding and env-auth logic.
3. **Upgrade the DE** to KDE Plasma 6 (Wayland) once the GPU path is confirmed.
4. **Decide permanent vs ephemeral** GPU pods to size the cost model.
