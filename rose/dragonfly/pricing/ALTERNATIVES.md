# Alternatives — a Lua approach and a Firecracker approach to the same problem

This document designs two *other* ways to solve the exact problem the WASM
pilot solves, so the choice can be judged on concrete shapes rather than
adjectives. See [CRITIQUE.md](CRITIQUE.md) for why the constraint
(*a fixed core that must safely run pricing code from other providers,
on the request hot path, from many languages*) is what it is.

Both alternatives are held to the **same contract** as the WASM plugins:

```
PricingRequest { sku, base_price_cents, quantity, segment? }
        ──►  provider logic  ──►
PricingQuote   { sku, quantity, unit_price_cents, subtotal_cents,
                 discount_cents, total_cents, currency, engine, applied_rules[] }
```

and the same routing job: the host picks a provider's engine per request and
falls back to a safe default on error.

---

## Approach A — Embedded Lua scripts

Lua is the classic "embed a scripting language in the host" answer: a tiny,
fast interpreter designed to be hosted inside a larger program (games, nginx,
Redis). A provider ships a `.lua` script; the host runs it in-process.

### The picture

```
  POST /price                ┌──────────────── host process ───────────────┐
  X-Provider: retail         │                                              │
 ───────────────────────►    │  resolve provider → lua chunk                │
                             │        │                                     │
                             │        ▼                                     │
                             │  ┌───────────────┐  retail.lua   (sandboxed) │
                             │  │ Lua runtime    │─ wholesale.lua  env       │
                             │  │ (one VM/chunk) │─ default.lua              │
                             │  └───────────────┘                           │
                             └──────────────────────────────────────────────┘
```

### How a provider ships logic

A plain text script exporting a `price(req)` function:

```lua
-- retail.lua
function price(req)
  local rules = {}
  local unit = math.floor(req.base_price_cents * 108 / 100)
  rules[#rules + 1] = "retail markup +8%"
  local subtotal = unit * req.quantity

  local pct = 0
  if req.quantity >= 50 then
    pct = 12; rules[#rules + 1] = "volume discount 12% (qty>=50)"
  elseif req.quantity >= 10 then
    pct = 5;  rules[#rules + 1] = "volume discount 5% (qty>=10)"
  end
  if req.segment == "vip" then
    pct = pct + 5; rules[#rules + 1] = "VIP segment +5%"
  end

  local discount = math.floor(subtotal * pct / 100)
  return {
    sku = req.sku, quantity = req.quantity, unit_price_cents = unit,
    subtotal_cents = subtotal, discount_cents = discount,
    total_cents = subtotal - discount, currency = "GBP",
    engine = "retail", applied_rules = rules,
  }
end
```

### How the host runs it

Embed a Lua runtime (e.g. `lupa`/LuaJIT from Python, or `mlua` from Rust),
load each provider's chunk once, and call `price` per request:

```python
from lupa import LuaRuntime

def load_engine(script_src: str):
    lua = LuaRuntime(unpack_returned_tuples=True, register_eval=False)
    g = lua.globals()
    # Attempt at a sandbox: remove the dangerous standard library.
    for name in ("os", "io", "require", "dofile", "loadfile",
                 "load", "loadstring", "package", "debug"):
        g[name] = None
    lua.execute(script_src)          # defines price()
    return g.price

# per request: quote = engines[provider](request_table)
```

### The sandbox story — and why it is the weak point

This is where Lua fails the primary condition. "Niling the dangerous globals"
is the textbook sandbox and it is **not enough** for untrusted code:

- **LuaJIT's FFI** can call arbitrary C — a single `ffi.cdef` + `ffi.C` escapes
  the entire sandbox. You must use plain Lua (not LuaJIT) or strip FFI, losing
  the speed that made LuaJIT attractive.
- **No fuel metering by default.** `while true do end` or `string.rep("x", 2^40)`
  hangs or OOMs the host. You must install a `debug.sethook` instruction
  counter and a memory ceiling yourself — bolt-on, easy to get wrong.
- **Shared heap, no memory isolation.** The script's tables live in the host
  process's memory. Isolation is "the interpreter has no bug," not a
  structural boundary. There is no equivalent of WASM's linear memory.
- **Capability removal is denylist, not allowlist.** You enumerate what to
  take away; miss one (`collectgarbage`, a metatable trick, a leaked upvalue)
  and the wall has a hole. WASM is the inverse: nothing is granted unless the
  host imports it.

### Verdict

Lua is in-process and very fast — it matches WASM on latency and beats it on
authoring simplicity and binary size. But its sandbox is a **denylist around a
shared heap with no built-in resource limits**, which is precisely the
"scripting languages embedded in the product… flexible but unsafe" case
`genisis.md` rejects. **Right tool when you control the authors** (first-party
rules, internal power-users, a trusted ops team); **wrong tool for untrusted
third-party code**, which is our actual constraint.

---

## Approach B — Firecracker microVMs

Firecracker is the microVM that powers AWS Lambda and Fargate: a minimal VMM
that boots a stripped Linux guest in ~125 ms with a real hardware-virtualised
boundary. Each provider's logic runs *inside its own VM*, in any language,
behind a KVM wall.

### The picture

```
  POST /price             ┌─────── host ───────┐
  X-Provider: retail      │  router / VM pool   │
 ───────────────────►     │        │            │
                          │        ▼ vsock RPC   │
                          │  ┌──────────────┐    │   ┌──────────────┐
                          │  │ retail µVM   │◄───┼──►│ wholesale µVM│
                          │  │  guest agent │    │   │  guest agent │
                          │  │  + provider  │    │   │  + provider  │
                          │  │    code      │    │   │    code      │
                          │  └──────────────┘    │   └──────────────┘
                          └─────────────────────┘   (separate kernels, KVM)
```

### How a provider ships logic

A provider ships an **executable** (any language) plus its deps, baked into a
guest root filesystem image. Inside the guest runs a small agent that speaks
the pricing contract over a socket:

```python
# guest_agent.py — runs inside the microVM, talks to the host over vsock
import json, socket
from provider_pricing import price          # the provider's own module

def serve(conn):
    req = json.loads(conn.recv(65536))
    conn.sendall(json.dumps(price(req)).encode())

# bind an AF_VSOCK socket, loop serve() per request
```

### How the host runs it

The host drives Firecracker over its REST API: configure a boot source
(kernel), a rootfs drive, a vsock device, then `InstanceStart`. Requests and
responses cross as JSON over **vsock** (host↔guest virtio socket — no real
network needed):

```jsonc
// firecracker machine config (per provider VM)
{
  "boot-source": { "kernel_image_path": "vmlinux", "boot_args": "console=off ..." },
  "drives": [{ "drive_id": "root", "path_on_host": "retail-rootfs.ext4",
               "is_root_device": true, "is_read_only": true }],
  "machine-config": { "vcpu_count": 1, "mem_size_mib": 128 },
  "vsock": { "guest_cid": 3, "uds_path": "/tmp/retail.vsock" }
}
```

### The isolation story — the strongest of the three

- **Hardware-virtualised boundary (KVM).** The guest has its *own kernel*. The
  Trusted Computing Base you must trust is the VMM + KVM, far smaller than a
  shared OS kernel (containers) and arguably stronger than a single WASM
  runtime, because a guest-kernel bug doesn't reach the host kernel.
- **Real resource limits.** vCPU count, memory size, and rate limiters are
  first-class machine config — not bolted on.
- **Read-only rootfs + no network device** unless you attach one: capability
  control at the device level.

### Hot path / latency — the catch

- **~125 ms cold boot.** Far too slow to spin up per request. You must keep a
  **pool of warm VMs per provider** (or use Firecracker **snapshots** to
  restore in single-digit ms) and route requests to a ready one.
- **Out-of-process on every call.** Even with a warm VM, each price is a vsock
  round-trip and a JSON (de)serialisation across the VM boundary —
  microseconds-to-millisecond overhead vs an in-process WASM call's
  sub-microsecond dispatch. This is the "webhook latency" tax `genisis.md`
  warns about, smaller but still present.
- **Per-VM memory floor.** Even a minimal guest reserves tens of MiB. With N
  providers × a warm pool each, RAM dominates. WASM instances are KiB.

### Verdict

Firecracker gives the **strongest isolation** and unrestricted language
choice. It is the right tool when provider logic is heavy, long-running, or
needs a real OS (a full Python/JVM stack, filesystem, subprocesses) and when
calls are coarse-grained (per batch / per session, not per row). For a
fine-grained pricing call on the request hot path it is the wrong *shape* —
you pay an out-of-process hop and a per-VM memory floor to isolate arithmetic
that finishes in microseconds.

---

## Side by side, against our constraint

| Axis | Embedded Lua | WASM (the pilot) | Firecracker µVM |
|---|---|---|---|
| Boundary strength | Weak (denylist, shared heap) | Strong (bytecode isolation, capability allowlist) | Strongest (KVM, own kernel) |
| Untrusted code? | **No** — sandbox leaks | **Yes** | **Yes** |
| Per-call latency | Sub-µs (in-proc) | Sub-µs (in-proc) | µs–ms (vsock hop) |
| Cold start | None | ms (instantiate) | ~125 ms (or ms via snapshot) |
| Memory per engine | KiB (shared heap) | KiB (linear mem) | tens of MiB per VM |
| Languages | Lua only | many (Rust/Go/C/AS…) | any (full OS) |
| Resource limits | Bolt-on (`sethook`) | Built-in (fuel, mem) | Built-in (vCPU/mem) |
| Ops complexity | Lowest | Low–medium | Highest (VM pools, kernels, rootfs) |
| Defence-in-depth role | — | fine, per-provider inner wall | coarse outer wall |

## Recommendation

These are not mutually exclusive — they sit at different grains:

- **Lua** — only if the authors are *trusted* (first-party rules, internal
  users). Fails our untrusted-provider constraint; documented for completeness.
- **WASM** — the per-call, per-provider isolation layer on the hot path:
  satisfies untrusted + in-process + polyglot simultaneously.
- **Firecracker** — the *coarse outer boundary* (run the whole WASM host inside
  a microVM for defence in depth), **or** the execution layer for providers
  whose logic is too heavy/stateful for WASM and is called coarsely.

A mature system likely runs **WASM inside Firecracker**: Firecracker isolates
the host and any heavyweight workloads at the VM grain; WASM isolates each
provider's hot-path pricing logic cheaply *within* it. Lua is the one this
problem rules out — useful to have written down precisely *because* it shows
where the "just embed a scripting language" instinct breaks.
