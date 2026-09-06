// Capture the mic as raw PCM the backend can feed straight into Whisper: mono,
// 16 kHz, little-endian f32. No MediaRecorder/webm, so the server needs no
// ffmpeg and no audio decoding — "as raw as possible."
//
// We pull samples via a ScriptProcessorNode (deprecated but universally
// supported and dependency-free), then resample to 16 kHz on stop for browsers
// that ignore the requested AudioContext rate (Safari).

const TARGET_RATE = 16000;

export class Recorder {
  private ctx: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private source: MediaStreamAudioSourceNode | null = null;
  private processor: ScriptProcessorNode | null = null;
  private chunks: Float32Array[] = [];
  private inputRate = TARGET_RATE;

  /// Request mic access and begin capturing samples. Throws if permission is denied.
  async start(): Promise<void> {
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1 },
    });
    // Ask for 16 kHz; Chrome/Firefox honour it, Safari won't — we resample on stop.
    this.ctx = new AudioContext({ sampleRate: TARGET_RATE });
    this.inputRate = this.ctx.sampleRate;
    this.source = this.ctx.createMediaStreamSource(this.stream);
    this.processor = this.ctx.createScriptProcessor(4096, 1, 1);
    this.chunks = [];

    this.processor.onaudioprocess = (e) => {
      // Copy — the event buffer is reused after this callback returns.
      this.chunks.push(new Float32Array(e.inputBuffer.getChannelData(0)));
    };

    this.source.connect(this.processor);
    // A silent sink keeps the graph pulling without echoing the mic to speakers.
    const sink = this.ctx.createGain();
    sink.gain.value = 0;
    this.processor.connect(sink);
    sink.connect(this.ctx.destination);
  }

  /// Stop recording, release the mic, and resolve with the clip as a Blob of raw
  /// little-endian f32 PCM at 16 kHz.
  async stop(): Promise<Blob> {
    this.processor?.disconnect();
    this.source?.disconnect();
    this.stream?.getTracks().forEach((t) => t.stop());
    await this.ctx?.close();
    this.ctx = null;
    this.processor = null;
    this.source = null;

    const samples = flatten(this.chunks);
    this.chunks = [];
    const at16k = resample(samples, this.inputRate, TARGET_RATE);
    // These bytes are the f32 PCM the backend reads. `at16k` is freshly allocated,
    // so its backing buffer is a plain ArrayBuffer (never shared) — the cast just
    // narrows TS 5.7's generic `ArrayBufferLike` to satisfy BlobPart.
    return new Blob([at16k.buffer as ArrayBuffer], { type: "application/octet-stream" });
  }
}

/// Concatenate captured chunks into one contiguous Float32Array.
function flatten(chunks: Float32Array[]): Float32Array {
  const total = chunks.reduce((n, c) => n + c.length, 0);
  const out = new Float32Array(total);
  let offset = 0;
  for (const c of chunks) {
    out.set(c, offset);
    offset += c.length;
  }
  return out;
}

/// Linear-resample mono PCM. A no-op copy when the rates already match.
function resample(input: Float32Array, from: number, to: number): Float32Array {
  if (from === to) return input.slice();
  const ratio = from / to;
  const out = new Float32Array(Math.floor(input.length / ratio));
  for (let i = 0; i < out.length; i++) {
    const src = i * ratio;
    const j = Math.floor(src);
    const frac = src - j;
    const a = input[j] ?? 0;
    const b = input[j + 1] ?? a;
    out[i] = a + (b - a) * frac;
  }
  return out;
}
