# hive

A deliberately tiny Axum service, deployed to Kubernetes with Helm as **multiple
identical pods behind one Service** — built so you can _watch_ horizontal scaling
work rather than take it on faith.

The code is almost beside the point. The lesson is in the shape of the
deployment: what it takes to run N copies of a web server safely, and when doing
so actually earns its keep.

```
rose/hive
├── src/main.rs          the app: replica-aware, probe-aware, drains on SIGTERM
├── Dockerfile           static musl binary on scratch (~small image, fast cold-start)
└── chart/               the Helm chart — one template per production concept
    ├── values.yaml        every knob, each mapped to a decision (read this)
    └── templates/
        ├── deployment.yaml   N pods, rolling updates, probes, Downward API
        ├── service.yaml      the stable front door + load balancer
        ├── hpa.yaml          autoscale on CPU
        └── pdb.yaml          protect availability during node drains
```

---

## The core question: why run more than one pod?

One process on one machine is the simplest thing that can serve traffic. Running
several identical copies is strictly more complicated — more moving parts, more
ways to be wrong. You take on that complexity to buy four specific dividends.

### 1. Availability — survive a failure without an outage

A single pod is a single point of failure. The node it sits on can lose power,
its kernel can panic, the kubelet can evict it. With one replica, any of those
is a full outage that lasts until a _new_ pod is scheduled, pulled, and warmed —
seconds to minutes of hard downtime.

With three replicas **spread across nodes**, losing one pod (or its whole node)
drops you to two. The Service stops routing to the dead pod the moment its
readiness probe fails, and the Deployment schedules a replacement in the
background. Callers see, at most, a couple of failed requests that a retry
papers over. The outage became a non-event.

> The "spread across nodes" part is load-bearing. Three replicas stacked on one
> node give you the _cost_ of three pods and the _availability_ of one. That's
> what `topologySpreadConstraints` in `deployment.yaml` prevents.

### 2. Zero-downtime deploys — replace the version while serving

You ship often. If shipping means "stop the old version, start the new one,"
every deploy is a small outage. Multiple pods let the Deployment roll the fleet
over _one batch at a time_: stand up a new-version pod, wait for it to pass
readiness, then retire an old one — repeat. `maxUnavailable: 0` guarantees the
healthy count never dips during the roll. Users never notice a deploy happened.

This is impossible with one pod: replacing it necessarily means a window with
zero serving pods.

### 3. Throughput — scale past what one process can do

A single process is bounded by one machine's CPU and memory. When steady-state
load exceeds what one pod can serve, you have two moves: a **bigger** pod
(vertical) or **more** pods (horizontal). Vertical hits a ceiling — the largest
node you can buy — and every resize is a restart. Horizontal scales linearly for
a stateless service and does it live: the HPA adds pods when average CPU climbs
and removes them when it falls. You serve the Monday-morning spike with ten pods
and the Sunday-night lull with three, paying only for what the load demands.

### 4. Elasticity of cost — shrink when idle

The flip side of throughput. Because the fleet can _shrink_, running multiple
pods isn't just "more expensive but safer." Under the HPA it tracks demand
downward too, back to the `minReplicas` floor, so you're not paying for a fleet
sized for peak during the 20 hours a day you're not at peak.

---

## When multiple pods earn their keep — and when they don't

Replication is not free. It only pays off under conditions worth checking before
you reach for it.

**Multiple pods earn their keep when:**

- **The workload is stateless** (or its state lives in a database / cache / object
  store, not in the pod). Any replica can serve any request. This is the single
  biggest prerequisite — everything above assumes it.
- **Uptime matters** enough that a single-pod outage would hurt: user-facing
  APIs, anything with an SLA, anything a paging alert is attached to.
- **You deploy often** and can't afford a maintenance window each time.
- **Load varies** — daily peaks, campaign spikes, unpredictable bursts. The gap
  between peak and trough is exactly the money autoscaling saves.

**Multiple pods are the wrong tool when:**

- **The work is a singleton by nature** — a leader-elected controller, a
  scheduler, a migration job, anything where two copies acting at once corrupts
  state. Run one, and make it _recoverable_, not replicated. (A `StatefulSet`
  with leader election is a different pattern.)
- **State lives in the pod** — in-memory sessions, a local SQLite file, an
  on-disk cache with no shared source of truth. Replicate that and requests to
  different pods see different data. Fix the statefulness first, or don't
  replicate.
- **Traffic is trivially low and steady** and the service isn't critical. Two
  small pods for basic redundancy is plenty; an HPA and a PDB are ceremony you
  won't use. Match the machinery to the stakes.
- **The bottleneck is downstream.** If every request hammers one database that's
  already saturated, adding app pods just adds pressure on the real constraint.
  More pods multiply a healthy tier; they can't fix an unhealthy dependency.

The honest summary: multiple pods buy **availability, safe deploys, and
elastic throughput** — and they cost you **statelessness discipline plus the
operational machinery below**. When the first list outweighs the second, run a
fleet. When it doesn't, one well-monitored pod is the simpler, correct choice.

---

## The machinery that makes a fleet safe

Running N copies is easy. Running them so that failures, deploys, and scaling
stay _invisible to clients_ is the actual work. Each concept below maps to a
place in the chart or the app.

| Concept                      | What it buys                                                          | Where                           |
| ---------------------------- | --------------------------------------------------------------------- | ------------------------------- |
| **Deployment + ReplicaSet**  | Declares "N pods of this version"; recreates any that die             | `deployment.yaml`               |
| **Service**                  | One stable address; load-balances across _ready_ pods only            | `service.yaml`                  |
| **Readiness probe**          | Gates traffic — an unready pod is pulled from rotation, not killed    | `deployment.yaml` → `/readyz`   |
| **Liveness probe**           | Restarts a _wedged_ pod — separate endpoint, forgiving thresholds     | `deployment.yaml` → `/healthz`  |
| **Rolling update**           | Replaces pods in batches; never drops below capacity                  | `strategy` in `deployment.yaml` |
| **Graceful shutdown**        | Drains in-flight requests on SIGTERM before exiting                   | `shutdown_signal` in `main.rs`  |
| **HPA**                      | Adds/removes pods to track a CPU target                               | `hpa.yaml`                      |
| **Resource requests/limits** | The scheduling + autoscaling contract; the OOM ceiling                | `resources` in `values.yaml`    |
| **Topology spread**          | Puts replicas on different nodes so one node ≠ SPOF                   | `deployment.yaml`               |
| **PodDisruptionBudget**      | Caps voluntary evictions so a node drain can't empty the fleet        | `pdb.yaml`                      |
| **Downward API**             | Injects the pod's own name/node so it can report who served a request | `env` in `deployment.yaml`      |

### The two probes are not the same question

The most common production mistake here is conflating them, so it's worth
spelling out — they point at different endpoints on purpose.

- **Liveness** (`/healthz`) asks _"is this process wedged?"_ Failing it **kills
  and restarts** the pod. It must check _only_ the process itself, never a
  dependency: if your database blips and your liveness probe checks the database,
  Kubernetes will kill every pod at once — removing all your capacity precisely
  when the dependency is already struggling. Keep it dumb and forgiving.
- **Readiness** (`/readyz`) asks _"should traffic come to me right now?"_ Failing
  it **pulls the pod from the Service**, but leaves it running to recover. This
  is the gate that makes warm-up and draining work: a booting pod fails readiness
  until its cache is warm; a shutting-down pod fails it to bleed off traffic.

### Graceful shutdown is the other half of a rolling update

When Kubernetes retires a pod it sends `SIGTERM` and, _concurrently_, removes the
pod from the Service endpoints. That removal is eventually-consistent — it takes
a moment to reach every node — so requests keep arriving for a beat after
SIGTERM. If the process exits immediately on SIGTERM, those requests are dropped,
and every rolling update sheds a few errors.

`hive` handles it the way production services should (`shutdown_signal` in
`main.rs`): on SIGTERM it flips readiness to false, **waits** for the endpoint
removal to propagate, and only then lets Axum's graceful shutdown finish the
in-flight requests and close the listener — all inside the pod's
`terminationGracePeriodSeconds` budget. That pause is why a deploy stays at
zero errors instead of "almost zero."

---

## Quick start

You need a cluster (local `kind`/`minikube`/`k3d`, or a real one), `kubectl`, and
`helm`. For the autoscaling demo you also need `metrics-server`.

```bash
# 1. Build the image. On kind/minikube, build it where the cluster can see it:
docker build -t hive:0.1.0 rose/hive
kind load docker-image hive:0.1.0            # kind
# minikube image load hive:0.1.0             # minikube

# 2. Install the chart.
helm install demo rose/hive/chart

# 3. Watch the fleet come up — each pod goes Ready before it takes traffic.
kubectl get pods -l app.kubernetes.io/instance=demo -w
```

### See load-balancing across replicas

```bash
kubectl port-forward svc/demo-hive 8080:80
# In another shell — the "pod" field cycles across the replicas:
for i in $(seq 12); do curl -s localhost:8080/ | grep -o '"pod":"[^"]*"'; done
```

### See self-healing

```bash
# Delete one pod and keep hitting the Service — traffic never stops.
kubectl delete pod "$(kubectl get pod -l app.kubernetes.io/instance=demo -o name | head -1)"
kubectl get pods -l app.kubernetes.io/instance=demo -w   # a replacement appears
```

### See a zero-downtime rolling update

```bash
# Rebuild with a new tag, then roll to it while a curl loop runs in another shell.
docker build -t hive:0.1.1 rose/hive && kind load docker-image hive:0.1.1
helm upgrade demo rose/hive/chart --set image.tag=0.1.1
kubectl rollout status deploy/demo-hive     # new pods pass readiness before old ones die
```

### See autoscaling (needs metrics-server)

```bash
kubectl get hpa demo-hive -w
# In another shell, drive CPU through the /work endpoint:
while true; do curl -s "localhost:8080/work?ms=500" >/dev/null; done
# Replicas climb toward maxReplicas; stop the load and, after the scale-down
# stabilization window, they ease back to minReplicas.
```

---

## Tuning notes

- **`replicaCount: 3`** is the smallest floor that keeps a healthy quorum while a
  rolling update is in flight _and_ survives losing one pod. Two is the bare
  minimum for redundancy; one is not redundant at all.
- **Resource `requests` drive both scheduling and the HPA.** Set them to the
  pod's honest steady-state usage. Too high → the pod looks idle and never scales
  up; too low → it looks saturated and scales up forever, or gets evicted under
  node pressure.
- **CPU limit loose, memory limit tight.** Exceeding the CPU limit only throttles
  the pod (graceful); exceeding the memory limit gets it OOM-killed (abrupt).
- **`terminationGracePeriodSeconds` must exceed** the in-app drain pause (5s) plus
  your longest realistic in-flight request. Too short and SIGKILL interrupts the
  drain you carefully set up.
- **Disable the HPA** (`--set autoscaling.enabled=false`) to pin a fixed
  `replicaCount` — the deployment omits `replicas` when the HPA owns it, so the
  two never fight.

---

_Part of `rose/` — a workspace of self-contained prototypes. `hive` is the
Kubernetes/Helm horizontal-scaling reference; it deploys real but does nothing
useful on purpose, so the deployment shape stays the whole lesson._
