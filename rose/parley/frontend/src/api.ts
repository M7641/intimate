// Backend client. One call per conversation turn.

export interface ConverseResponse {
  session: string;
  transcript: string;
  reply: string;
  audio: string | null; // base64 WAV
}

/// Send a recorded clip and the current session id; get back transcript, reply,
/// and reply audio. `session` is empty on the first turn — the backend mints one.
export async function converse(audio: Blob, session: string): Promise<ConverseResponse> {
  const form = new FormData();
  if (session) form.append("session", session);
  // Raw f32 PCM (16 kHz mono) — see recorder.ts. The backend reads the bytes
  // straight into Whisper; the filename is cosmetic.
  form.append("audio", audio, "clip.pcm");

  const res = await fetch("/api/converse", { method: "POST", body: form });
  if (!res.ok) {
    throw new Error(`backend ${res.status}: ${await res.text()}`);
  }
  return res.json();
}

/// Turn the base64 WAV from the backend into a playable object URL.
export function audioUrl(base64: string): string {
  const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
  const blob = new Blob([bytes], { type: "audio/wav" });
  return URL.createObjectURL(blob);
}

export interface VocabItem {
  word: string;
  count: number;
  last_seen: number;
}

export interface Progress {
  level: string;
  distinct_words: number;
  top_words: VocabItem[];
  practiced_areas: string[];
  next_area: string | null;
}

/// Read the learner's durable memories, reflected back for the Progress page.
export async function getProgress(): Promise<Progress> {
  const res = await fetch("/api/progress");
  if (!res.ok) {
    throw new Error(`backend ${res.status}: ${await res.text()}`);
  }
  return res.json();
}

export interface Sense {
  part_of_speech: string;
  gloss: string;
}

export interface Definition {
  word: string;
  source: string;
  source_url: string;
  senses: Sense[];
}

export interface SavedWord {
  word: string;
  count: number;
  last_seen: number;
  saved: boolean;
  definition: Definition | null;
}

/// Save a word and collect its definition from the trusted dictionary. Returns the
/// stored item; `definition` is null if the word was not found.
export async function saveWord(word: string): Promise<SavedWord> {
  const res = await fetch("/api/words", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ word }),
  });
  if (!res.ok) {
    throw new Error(`backend ${res.status}: ${await res.text()}`);
  }
  return res.json();
}

/// List the learner's saved words, with definitions, newest first.
export async function listWords(): Promise<SavedWord[]> {
  const res = await fetch("/api/words");
  if (!res.ok) {
    throw new Error(`backend ${res.status}: ${await res.text()}`);
  }
  return res.json();
}
