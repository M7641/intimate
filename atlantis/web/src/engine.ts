// Glue over the wasm ABI (wasm/src/lib.rs). Each call writes JSON, or a
// panic message, into one buffer; we read it back after the call.

import type { Action, HeroSheet, Snapshot } from "./types";

type Exports = {
  memory: WebAssembly.Memory;
  init: () => void;
  out_ptr: () => number;
  out_len: () => number;
  alloc: (len: number) => number;
  catalog: () => void;
  game_new: (seed: bigint) => void;
  game_apply: (ptr: number, len: number) => number;
  game_step: (policy: number) => void;
};

export type Policy = "random" | "brawler";

export type Engine = {
  catalog: () => HeroSheet[];
  newGame: (seed: number) => Snapshot;
  apply: (action: Action) => Snapshot;
  step: (policy: Policy) => Snapshot;
};

export async function loadEngine(url = "/atlantis.wasm"): Promise<Engine> {
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url), {});
  const ex = instance.exports as unknown as Exports;
  ex.init();

  // A fresh view every time: memory.buffer is replaced when memory grows.
  const readOut = () =>
    new TextDecoder().decode(
      new Uint8Array(ex.memory.buffer, ex.out_ptr(), ex.out_len()),
    );

  const writeIn = (text: string): [number, number] => {
    const bytes = new TextEncoder().encode(text);
    const ptr = ex.alloc(bytes.length);
    new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes);
    return [ptr, bytes.length];
  };

  const call = <T>(f: () => number | void): T => {
    let code: number | void;
    try {
      code = f();
    } catch {
      throw new Error(`engine panic: ${readOut()}`);
    }
    if (code === 1) throw new Error(readOut());
    return JSON.parse(readOut()) as T;
  };

  return {
    catalog: () => call<HeroSheet[]>(() => ex.catalog()),
    newGame: (seed) => call<Snapshot>(() => ex.game_new(BigInt(seed))),
    apply: (action) =>
      call<Snapshot>(() => {
        const [ptr, len] = writeIn(JSON.stringify(action));
        return ex.game_apply(ptr, len);
      }),
    step: (policy) => call<Snapshot>(() => ex.game_step(policy === "random" ? 0 : 1)),
  };
}
