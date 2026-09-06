# Infrastructure Insights

Key takeaways from the infrastructure exploration — patterns and mental models that are easy to forget.

---

## Why Erlang Can Hot-Swap But Docker Cannot

Erlang's OTP was designed from the ground up for hot code reloading — the BEAM VM manages two versions of a module simultaneously, with per-process routing. Docker, by contrast, is built on immutability: the filesystem is a fixed snapshot from build time. Grafting hot-swap onto a container is like trying to do live reload on a statically compiled binary — technically possible, but the entire system fights against you. The Incu idea (in-container updates) was inspired by Erlang, but containers are not that VM.

---

## The "Stub Sources" Pattern for Cargo Docker Caching

The trick in `cocoon/data_view/Dockerfile`:

```dockerfile
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release          # Caches ~400+ dependency crates
RUN rm -rf target/release/.fingerprint/data_view-*
COPY src/ src/                     # Only this layer changes day-to-day
RUN cargo build --release          # Rebuilds only your code (~30s vs ~10min)
```

This exploits the fact that Cargo caches compiled dependencies per-crate. By forcing a first build with empty sources, you capture all dependency compilation in a Docker layer. On subsequent builds, only your project code (~100 lines) is recompiled. The equivalent Python pattern:

```dockerfile
COPY pyproject.toml uv.lock ./
RUN uv sync --locked --no-install-project
COPY src/ src/
RUN uv sync --locked
```

Same principle: dependencies change rarely, source changes constantly. Order Dockerfile instructions accordingly.

---

## The 5 → 17 Rule

When someone lists the components of a production service, they list the _application_ components — the things users interact with. But beneath each visible component, there are 2-3 invisible components that make it work:

| Visible | Invisible beneath it |
|---------|---------------------|
| API | ECS Fargate, ALB, security group, IAM task role, health checks |
| Database | VPC private subnet, security group, Secrets Manager, automated backups |
| Web app | S3, CloudFront, ACM certificate, Route 53 |
| Scheduled tasks | EventBridge Scheduler, ECS RunTask, CloudWatch Logs |
| API Gateway | WAF, usage plans, rate limiting config |

This is what managed platforms (Nimbus, Heroku, Railway, Render) sell: you think about the 5, they own the 17. Moving to raw AWS means owning every row in that table.

---

## Reusable Modules Mirror ouroboros

The Terraform pattern of a reusable `ecs_service` module called 3 times (API, webapp, worker) with different parameters reflects exactly what ouroboros does when `build_service()` accepts a `service_type` parameter. The difference: ouroboros hides the complexity behind a REST API; Terraform exposes it in HCL. Both use the same principle — a parameterisable abstraction instantiated N times with different values.

```python
# ouroboros — one function, multiple service types
deploy_service(service_name="my-api", service_type="api", target="prod")
deploy_service(service_name="my-app", service_type="webapp", target="prod")
```

```hcl
# Terraform — one module, multiple instances
module "api"    { source = "../modules/ecs_service"; service_name = "api";    ... }
module "webapp" { source = "../modules/ecs_service"; service_name = "webapp"; ... }
module "worker" { source = "../modules/ecs_service"; service_name = "worker"; ... }
```

Same idea, different syntax.

---

## NAT Gateway: The Hidden Cost Trap

At $0.045/GB of data processed, a NAT Gateway can cost more than your containers if you have significant outbound traffic (pulling Docker images, calling external APIs, downloading data). The fix: VPC endpoints.

- **S3 Gateway Endpoint**: free, eliminates NAT charges for S3 traffic
- **ECR, Secrets Manager, CloudWatch Logs Interface Endpoints**: ~$0.01/hr each, but eliminate NAT data processing charges for those services

This is the first cost optimisation to make on any AWS setup. Most teams discover it after the first bill.

---

## Immutable Infrastructure vs In-Container Updates

The core argument: containers provide a guarantee that **what you test is what runs in production**. In-container updates break this by mutating the running state after deployment. If the container crashes and restarts, you get the old image — not the updated version.

The real problem Incu was solving was not that containers are inflexible — it was that deployments felt slow and manual. That is a CI/CD problem, not a container problem. Automate the pipeline (`push → build → deploy`), and the friction disappears without breaking the container model.

Legitimate self-update contexts: desktop apps (Electron), edge/IoT devices, air-gapped environments — anywhere the deployment mechanism is genuinely unavailable. On a cloud platform, the deployment mechanism _is_ the product.
