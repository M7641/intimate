# moss

Axum HTTP gateway proxying requests to a tonic gRPC "ML" service.

## Layout

```
proto/math.proto          schema (single source of truth)
build.rs                  invokes tonic-build at compile time
src/lib.rs                re-exports generated proto types
src/bin/math_server.rs    gRPC server (toy linear model)
src/bin/gateway.rs        HTTP -> gRPC proxy
```

## Run locally

Two terminals:

```sh
cargo run --bin math_server
cargo run --bin gateway
```

Or with Docker (single image, two services):

```sh
docker compose up --build
```

Then in either case:

```sh
curl -s localhost:3000/add -H 'content-type: application/json' \
  -d '{"a": 1.5, "b": 2.5}'
# {"result":4.0}

curl -s localhost:3000/predict -H 'content-type: application/json' \
  -d '{"features": [1.0, 2.0, 3.0]}'
# {"prediction":2.9,"model_version":"v0.1.0-toy"}
```

## When this pattern pays off

- Multiple internal services need to call each other -> shared `.proto` files give
  you typed, versioned contracts and identical client/server code.
- The edge needs to stay HTTP/JSON for browsers, webhooks, curl.
- gRPC's HTTP/2 multiplexing keeps internal RPCs cheap at high fan-out.

If you have one service: skip gRPC, just use Axum end-to-end.

## Configuration

Both binaries read environment variables (defaults shown):

| Var                | Used by        | Default                     |
| ------------------ | -------------- | --------------------------- |
| `BIND_ADDR`        | both           | `127.0.0.1:50051` / `:3000` |
| `MATH_SERVICE_URL` | gateway        | `http://127.0.0.1:50051`    |
| `RUST_LOG`         | both (tracing) | unset                       |

## Deployment shape

The Dockerfile builds **one image** containing both binaries; compose (or k8s)
runs two containers from it, picking the entrypoint via `command:`. In
Kubernetes this becomes:

- one `Deployment` per binary (math_server, gateway)
- one `Service` ClusterIP for `math_server` (internal only)
- one `Service` LoadBalancer/Ingress for `gateway` (public)

Same image artifact, two runtime configurations.
