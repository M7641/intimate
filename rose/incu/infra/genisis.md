# AWS Infrastructure — From Managed Platform to Raw Cloud

The repo currently deploys through Nimbus via the `ouroboros` module. Nimbus abstracts everything below the container level — networking, load balancing, SSL, scaling, scheduling, image lifecycle — into a handful of Python function calls. This document maps out what lives beneath that abstraction: a complete AWS infrastructure for a production service, built with modern tooling.

Understanding this matters whether you stay on Nimbus or not. It changes how you think about failure modes, cost, security boundaries, and system design.

---

## 1. The Complete Picture

### Architecture Diagram

```
                              Internet
                                 │
                    ┌────────────┼────────────┐
                    │        Route 53          │
                    │         (DNS)            │
                    └─────┬──────────┬─────────┘
                          │          │
              ┌───────────▼──┐  ┌────▼──────────────┐
              │  CloudFront  │  │   API Gateway      │
              │    (CDN)     │  │  (rate limit, auth) │
              └───────┬──────┘  └────────┬───────────┘
                      │                  │
              ┌───────▼──────┐  ┌────────▼───────────┐
              │  S3 Bucket   │  │   ALB              │
              │  (React SPA) │  │  (load balancer)   │
              └──────────────┘  └────────┬───────────┘
                                         │
┌────────────────────────────────────────────────────────────────┐
│  VPC                                                           │
│                                                                │
│  ┌─ Public Subnets (2 AZs) ──────────────────────────────┐    │
│  │  ALB nodes          NAT Gateway                        │    │
│  └────────────────────────┬───────────────────────────────┘    │
│                           │                                    │
│  ┌─ Private Subnets (2 AZs) ─────────────────────────────┐    │
│  │                                                        │    │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐     │    │
│  │  │ ECS Task │  │ ECS Task │  │ ECS Task         │     │    │
│  │  │  (API)   │  │ (Webapp) │  │ (Worker/Cron)    │     │    │
│  │  └──────────┘  └──────────┘  └──────────────────┘     │    │
│  │                                                        │    │
│  │  ┌──────────────────┐  ┌────────────────────────┐     │    │
│  │  │ RDS PostgreSQL   │  │ ElastiCache (optional) │     │    │
│  │  │ (Multi-AZ)       │  │                        │     │    │
│  │  └──────────────────┘  └────────────────────────┘     │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                │
│  VPC Endpoints: S3, ECR, Secrets Manager, CloudWatch Logs      │
└────────────────────────────────────────────────────────────────┘

  ┌───────────────────────────────────────────────────────┐
  │  Supporting Services                                   │
  │  ECR (container registry)    Secrets Manager           │
  │  EventBridge Scheduler       SQS (async queues)        │
  │  ACM (SSL certificates)      CloudWatch (monitoring)   │
  │  WAF (firewall)              IAM (access control)      │
  └───────────────────────────────────────────────────────┘
```

### What You Listed vs What You Actually Need

Your list of 5 components sits inside a much larger system:

| Layer | Your list | What's also needed |
|-------|-----------|-------------------|
| **Foundation** | — | VPC, subnets, security groups, NAT Gateway, IAM |
| **Data** | RDS | S3, Secrets Manager |
| **Compute** | API, Webapp, Workflows | ECR, ECS Fargate (the thing that _runs_ them) |
| **Traffic** | API Gateway | ALB, CloudFront, Route 53, ACM |
| **Async** | Workflows (periodic) | SQS, EventBridge (scheduling + events) |
| **Observability** | — | CloudWatch or OTel stack |
| **Security** | — | WAF, VPC endpoints, encryption |
| **CI/CD** | — | GitHub Actions → ECR → ECS |

5 becomes ~17. This is what a managed platform abstracts away.

---

## 2. The Components

### 2.1 Foundation — VPC, Subnets, Security Groups, IAM

**VPC** is the network boundary. Everything runs inside it. Nothing outside can reach anything inside unless you explicitly allow it.

**Subnets** split the VPC into zones across two availability zones (for redundancy):
- **Public subnets** (2): hold the ALB and NAT Gateway. These have internet access.
- **Private subnets** (2): hold ECS tasks, RDS, and anything that should not be directly reachable from the internet.

**Security groups** are stateful firewalls attached to each resource. They form a chain:

```
Internet → ALB (allows 443 inbound)
              → ECS tasks (allows traffic only from ALB)
                   → RDS (allows 5432 only from ECS tasks)
```

Each layer only accepts traffic from the layer above it. If the API is compromised, the attacker cannot reach RDS directly — they must go through the application.

**NAT Gateway** sits in the public subnet. Private resources (ECS tasks) route outbound traffic through it to reach the internet (pulling images, calling external APIs) without being reachable from the internet.

**IAM** defines what each component can do:
- **Task execution role**: ECS uses this to pull images from ECR and read secrets from Secrets Manager. It is the "start the container" role.
- **Task role**: the container itself uses this at runtime to access S3, SQS, or other AWS services. It is the "run the application" role.

Principle: each role has the minimum permissions needed. An API container that reads from S3 and writes to SQS gets exactly those two permissions, nothing more.

### 2.2 Data — RDS, S3, Secrets Manager

**RDS PostgreSQL** is the relational database. Runs in the private subnet — no public internet access. Key decisions:
- **Multi-AZ**: AWS maintains a standby replica in a second availability zone. If the primary fails, automatic failover in ~60 seconds. Non-negotiable for production.
- **Automated backups**: point-in-time recovery within the retention window (default 7 days, extend to 35 for production).
- **Instance class**: `db.t4g.medium` for small workloads (2 vCPU, 4GB RAM, ~$65/month). Scale up when needed.
- **Connection**: via security group, not public endpoint. The connection string lives in Secrets Manager, not in environment variables.

**S3** serves multiple purposes:
- Origin for CloudFront (hosting the built React SPA)
- Artifact storage (file uploads, exports, ML model artifacts)
- Terraform remote state backend (see Section 3)

**Secrets Manager** stores database credentials, API keys, and any sensitive configuration. ECS tasks read secrets at startup via the task execution role. This replaces the `os.getenv("API_KEY")` pattern in ouroboros — instead of setting environment variables manually, the container fetches them from Secrets Manager on boot.

### 2.3 Compute — ECR, ECS Fargate

**ECR** (Elastic Container Registry) is a private Docker registry. CI pushes images here; ECS pulls from here. The existing multi-stage Dockerfiles (`cocoon/data_view/Dockerfile` with its 3-stage Bun+Rust+runtime pattern) work directly with ECR — no changes needed.

**ECS Fargate** runs containers without managing EC2 instances. You define a task (CPU, memory, image, env vars, ports) and a service (how many copies, health checks, scaling). Fargate handles the rest.

Three service types, mapping directly to what ouroboros already deploys:

| Service | ouroboros equivalent | ECS pattern |
|---------|---------------------|-------------|
| API (Axum/FastAPI) | `deploy_service(service_type="api")` | ECS service behind ALB, min 2 tasks |
| Web app | `deploy_service(service_type="webapp")` | ECS service behind ALB, or static S3+CloudFront |
| Workers/Cron | `deploy_workflows()` | EventBridge Scheduler → ECS RunTask |

**Task definition** (the blueprint for a container):

```json
{
  "cpu": "512",
  "memory": "1024",
  "containerDefinitions": [{
    "name": "api",
    "image": "123456789.dkr.ecr.eu-west-1.amazonaws.com/my-api:v1.2.3",
    "portMappings": [{"containerPort": 8050}],
    "secrets": [
      {"name": "DATABASE_URL", "valueFrom": "arn:aws:secretsmanager:..."}
    ],
    "logConfiguration": {
      "logDriver": "awslogs",
      "options": {"awslogs-group": "/ecs/my-api"}
    }
  }]
}
```

Compare to `build_service()` in ouroboros where `instanceTypeId: 52` selects a fixed instance type — on raw AWS, you choose vCPU and memory directly.

**Auto-scaling**: ECS scales based on CPU, memory, or custom CloudWatch metrics. The ouroboros equivalents are `minInstances: 2` for prod and `scaleToZero: True` for non-prod — ECS supports both patterns via target tracking policies and scale-to-zero with Application Auto Scaling.

### 2.4 Traffic — ALB, API Gateway, CloudFront, Route 53, ACM

**ALB** (Application Load Balancer) distributes traffic across ECS tasks. It runs health checks (`GET /health` every 30s) and removes unhealthy tasks from rotation. Path-based routing can direct `/api/*` to one service and `/admin/*` to another.

**API Gateway** sits in front of the ALB for public-facing APIs. It provides:
- Rate limiting (protect against traffic spikes)
- API key management (per-consumer access control)
- Request/response validation
- Usage plans and throttling
- For internal services, skip API Gateway and route directly through ALB.

**CloudFront** is a CDN for the React SPA. The built frontend (Bun/Vite output from `frontend/dist/`) is uploaded to S3 and served globally through CloudFront edge locations. Users in Tokyo and London both get fast load times.

**Route 53** manages DNS. An A-record alias points `app.example.com` to CloudFront and `api.example.com` to the ALB.

**ACM** (Certificate Manager) provides free SSL certificates with automatic renewal. Attach them to ALB and CloudFront — HTTPS everywhere with zero maintenance.

### 2.5 Async — EventBridge Scheduler, SQS

**EventBridge Scheduler** replaces Nimbus's workflow scheduling. Instead of `deploy_workflows()` with cron specs embedded in the workflow spec, you define a schedule rule that triggers an ECS RunTask:

```
"every 6 hours" → EventBridge rule → ECS RunTask → worker container starts → runs job → exits
```

This is the same as ouroboros iterating over `workflow_specs` and assigning `imageId` per step — but managed by AWS instead of Nimbus.

**SQS** (Simple Queue Service) decouples services. The pattern:
1. API receives a request (e.g., "generate report")
2. API puts a message on the SQS queue and returns 202 Accepted immediately
3. Worker polls the queue, processes the message, stores the result
4. If the worker crashes, the message becomes visible again after a timeout (automatic retry)
5. After N failures, the message moves to a dead letter queue for investigation

This is essential for any operation that takes longer than an HTTP request should last.

### 2.6 Observability

Two paths:

**Path A — CloudWatch (native, zero setup)**
- Logs: ECS tasks send stdout/stderr to CloudWatch Logs automatically via the `awslogs` driver
- Metrics: ECS, RDS, ALB all emit metrics to CloudWatch (CPU, memory, request count, error rate, latency)
- Alarms: "if ALB 5xx rate > 5% for 5 minutes, send SNS notification"
- Dashboards: built-in, adequate for most needs

**Path B — OTel → Grafana Cloud (richer, familiar)**
The Apocrypha stack (`rose/apocrypha/`) translates almost directly to AWS:
- OTel Collector runs as an ECS sidecar container alongside your application
- Exports traces to Grafana Tempo, logs to Loki, metrics to Prometheus — all hosted on Grafana Cloud
- Same dashboards, same query language, same experience as local development
- More powerful than CloudWatch for distributed tracing and correlation

Start with CloudWatch (free tier is generous). Move to Path B when you need cross-service tracing or outgrow CloudWatch's querying.

### 2.7 Security

**WAF** (Web Application Firewall) attaches to API Gateway or ALB. Provides:
- SQL injection protection
- Rate limiting by IP
- Geographic restrictions
- Bot detection
- AWS managed rule sets handle the common threats out of the box.

**VPC Endpoints** allow ECS tasks to access AWS services (S3, ECR, Secrets Manager, CloudWatch Logs) without routing through the NAT Gateway. This is cheaper (no NAT data processing charges) and faster (stays within the AWS network).

**Encryption**: RDS encrypts data at rest by default. S3 encrypts objects by default. HTTPS via ACM encrypts data in transit. These are all on by default in modern AWS — you have to actively turn them off to be insecure.

### 2.8 CI/CD — GitHub Actions

The pipeline mirrors the ouroboros deploy flow, but targeting ECR + ECS instead of Nimbus:

```yaml
name: Deploy

on:
  push:
    branches: [main]
  release:
    types: [published]

env:
  AWS_REGION: eu-west-1
  ECR_REPO: my-api

jobs:
  deploy:
    runs-on: ubuntu-latest
    permissions:
      id-token: write    # OIDC auth to AWS (no long-lived keys)
      contents: read

    steps:
      - uses: actions/checkout@v4

      - name: Configure AWS credentials (OIDC)
        uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: arn:aws:iam::123456789:role/github-actions
          aws-region: ${{ env.AWS_REGION }}

      - name: Login to ECR
        uses: aws-actions/amazon-ecr-login@v2

      - name: Build and push image
        run: |
          docker build -t $ECR_REPO:${{ github.sha }} .
          docker tag $ECR_REPO:${{ github.sha }} \
            123456789.dkr.ecr.$AWS_REGION.amazonaws.com/$ECR_REPO:${{ github.sha }}
          docker push \
            123456789.dkr.ecr.$AWS_REGION.amazonaws.com/$ECR_REPO:${{ github.sha }}

      - name: Deploy to ECS
        run: |
          aws ecs update-service \
            --cluster my-cluster \
            --service my-api \
            --force-new-deployment
```

Key difference from ouroboros: authentication uses OIDC (no long-lived API keys stored as secrets) and the deploy is a single `aws ecs update-service` call. ECS handles rolling deployment automatically — starts new tasks, health checks them, drains old tasks.

---

## 3. Modern Tooling — Terraform

### Why Terraform

Terraform is the industry standard for infrastructure-as-code:

- **Declarative**: describe the desired state, Terraform figures out what to create, update, or delete to get there.
- **Plan/Apply**: `terraform plan` shows exactly what will change before anything happens. No surprises.
- **State management**: Terraform tracks what it has created. If you delete a resource from the code, it deletes it from AWS.
- **Language-agnostic**: HCL (HashiCorp Configuration Language) is purpose-built for infrastructure. Fits a polyglot repo (Rust + Python + React) without favouring any language.
- **Ecosystem**: the AWS provider covers every service. Community modules handle common patterns (VPC, ECS, RDS) with battle-tested defaults.
- **OpenTofu**: open-source fork of Terraform, fully compatible, if HashiCorp's BSL licence is a concern.

### Project Structure

```
infra/
├── modules/
│   ├── networking/          # VPC, subnets, security groups, NAT
│   │   ├── main.tf
│   │   ├── variables.tf
│   │   └── outputs.tf
│   ├── database/            # RDS
│   ├── ecs_service/         # Reusable for API, webapp, worker
│   ├── cdn/                 # CloudFront + S3
│   └── api_gateway/
├── environments/
│   ├── dev/
│   │   ├── main.tf          # Calls modules with dev-specific values
│   │   └── terraform.tfvars
│   ├── staging/
│   └── prod/
├── backend.tf               # Remote state (S3 + DynamoDB)
└── versions.tf              # Provider version pins
```

Each module is reusable. The `ecs_service` module is called three times — once for the API, once for the webapp, once for the worker — with different parameters each time.

### Remote State

Terraform state must be stored remotely so the team shares a single source of truth:

```hcl
# backend.tf
terraform {
  backend "s3" {
    bucket         = "my-project-terraform-state"
    key            = "env/dev/terraform.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "terraform-locks"  # Prevents concurrent applies
    encrypt        = true
  }
}
```

### VPC Module (condensed)

```hcl
resource "aws_vpc" "main" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
}

resource "aws_subnet" "private" {
  count             = 2
  vpc_id            = aws_vpc.main.id
  cidr_block        = cidrsubnet(aws_vpc.main.cidr_block, 8, count.index)
  availability_zone = data.aws_availability_zones.available.names[count.index]
}

resource "aws_subnet" "public" {
  count                   = 2
  vpc_id                  = aws_vpc.main.id
  cidr_block              = cidrsubnet(aws_vpc.main.cidr_block, 8, count.index + 100)
  availability_zone       = data.aws_availability_zones.available.names[count.index]
  map_public_ip_on_launch = true
}

resource "aws_nat_gateway" "main" {
  allocation_id = aws_eip.nat.id
  subnet_id     = aws_subnet.public[0].id
}
```

### ECS Service Module (reusable)

```hcl
variable "service_name" { type = string }
variable "image"        { type = string }
variable "cpu"          { type = number, default = 512 }
variable "memory"       { type = number, default = 1024 }
variable "port"         { type = number, default = 8050 }
variable "desired_count" { type = number, default = 2 }
variable "secrets"      { type = map(string), default = {} }

resource "aws_ecs_task_definition" "this" {
  family                   = var.service_name
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = var.cpu
  memory                   = var.memory
  execution_role_arn       = aws_iam_role.execution.arn
  task_role_arn            = aws_iam_role.task.arn

  container_definitions = jsonencode([{
    name      = var.service_name
    image     = var.image
    essential = true
    portMappings = [{ containerPort = var.port }]
    secrets = [for k, v in var.secrets : { name = k, valueFrom = v }]
    logConfiguration = {
      logDriver = "awslogs"
      options   = { "awslogs-group" = "/ecs/${var.service_name}" }
    }
  }])
}

resource "aws_ecs_service" "this" {
  name            = var.service_name
  cluster         = var.cluster_id
  task_definition = aws_ecs_task_definition.this.arn
  desired_count   = var.desired_count
  launch_type     = "FARGATE"

  network_configuration {
    subnets         = var.private_subnet_ids
    security_groups = [var.service_sg_id]
  }

  load_balancer {
    target_group_arn = var.target_group_arn
    container_name   = var.service_name
    container_port   = var.port
  }
}
```

Called three times:

```hcl
module "api" {
  source       = "../modules/ecs_service"
  service_name = "api"
  image        = "123456789.dkr.ecr.eu-west-1.amazonaws.com/api:latest"
  cpu          = 512
  memory       = 1024
  port         = 8050
}

module "webapp" {
  source       = "../modules/ecs_service"
  service_name = "webapp"
  image        = "123456789.dkr.ecr.eu-west-1.amazonaws.com/webapp:latest"
  cpu          = 256
  memory       = 512
  port         = 3000
}

module "worker" {
  source        = "../modules/ecs_service"
  service_name  = "worker"
  image         = "123456789.dkr.ecr.eu-west-1.amazonaws.com/worker:latest"
  cpu           = 1024
  memory        = 2048
  desired_count = 1
}
```

### RDS Module (condensed)

```hcl
resource "aws_db_instance" "main" {
  identifier           = "my-project-db"
  engine               = "postgres"
  engine_version       = "16.4"
  instance_class       = "db.t4g.medium"
  allocated_storage    = 50
  multi_az             = true
  db_subnet_group_name = aws_db_subnet_group.main.name
  vpc_security_group_ids = [var.db_sg_id]

  username                    = "app"
  manage_master_user_password = true  # AWS generates and stores in Secrets Manager

  backup_retention_period = 14
  skip_final_snapshot     = false
  storage_encrypted       = true
}
```

### Alternatives to Terraform

| Tool | Strengths | Trade-offs |
|------|-----------|------------|
| **AWS CDK** | Write infrastructure in TypeScript or Python. Same languages as the app code. | Locked to AWS. Compiles to CloudFormation (verbose, slower applies). |
| **Pulumi** | Real programming languages. Multi-cloud. Strong typing. | Smaller ecosystem than Terraform. State management requires Pulumi Cloud or self-hosted backend. |
| **SST** | Excellent developer experience. Live Lambda dev. Opinionated and fast. | Focused on serverless. Less mature for container workloads. |

Terraform has the largest community, the most battle-tested modules, and the widest hiring pool familiarity. For a team starting from zero IaC, it is the safest default. Re-evaluate if the HCL syntax becomes a bottleneck — CDK or Pulumi let you use the languages you already know.

---

## 4. Step-by-Step Cooking Guide

These steps are ordered by dependency — each builds on the previous.

### Step 1 — Terraform + Remote State
Create the S3 bucket and DynamoDB table for state management. This is bootstrapped manually (the only manual step) because Terraform cannot manage its own state backend before it exists.

### Step 2 — VPC + Networking
Create the VPC, subnets, NAT Gateway, internet gateway, route tables, and base security groups. Everything else depends on this.

### Step 3 — ECR + Push First Image
Create the ECR repository. Build and push an existing Dockerfile (`cocoon/data_view/Dockerfile` is production-ready). Validate the image appears in the registry.

### Step 4 — RDS
Create the PostgreSQL instance in the private subnet. Store credentials in Secrets Manager. Test connectivity from your local machine via an SSH bastion or VPN (not by making RDS public).

### Step 5 — ECS Service (API first)
Create the ECS cluster, task definition, and service for the API. Start with 1 task, validate it starts, reads secrets, and connects to RDS. This is the "hello world" of the infrastructure.

### Step 6 — ALB
Create the ALB in public subnets. Add a target group pointing to the ECS API service. Configure health checks on `/health`. Attach an ACM certificate for HTTPS.

### Step 7 — API Gateway (optional)
If the API is public-facing: add API Gateway in front of the ALB with rate limiting and API key management. If internal only: skip this step, the ALB is sufficient.

### Step 8 — Webapp
For a React SPA: create an S3 bucket, upload the built frontend, create a CloudFront distribution with the S3 origin. For SSR: deploy a second ECS service. CloudFront + S3 is simpler and cheaper for SPAs.

### Step 9 — Scheduled Tasks
Create EventBridge Scheduler rules that trigger ECS RunTask for periodic workflows. Define the schedule (cron or rate expression), target (ECS cluster + task definition), and the network configuration (same VPC, same subnets as the services).

### Step 10 — DNS + SSL
Create Route 53 hosted zone. Add A-record aliases: `app.example.com` → CloudFront, `api.example.com` → ALB. Request ACM certificates and attach to ALB + CloudFront.

### Step 11 — Monitoring + Alerting
Set up CloudWatch alarms: ECS task failures, RDS CPU > 80%, ALB 5xx rate > 5%, disk storage low. Create an SNS topic for notifications. Optionally: add OTel Collector as a sidecar for richer observability (traces, structured logs).

### Step 12 — Security Hardening
Add WAF to API Gateway/ALB. Create VPC endpoints for S3, ECR, Secrets Manager, CloudWatch Logs (reduces NAT Gateway costs). Audit IAM policies for least privilege. Enable AWS Config rules for compliance drift detection.

---

## 5. Cost Awareness

Approximate monthly cost for a small production setup (eu-west-1):

| Component | Monthly Estimate | Notes |
|-----------|-----------------|-------|
| ECS Fargate (3 services, 0.5 vCPU, 1GB each) | $80–120 | 24/7, no scale-to-zero |
| RDS PostgreSQL (db.t4g.medium, Multi-AZ) | $130–180 | Storage + IOPS extra |
| ALB | $25 | + $0.008 per LCU-hour |
| NAT Gateway | $45 | + $0.045/GB processed |
| CloudFront | $5–20 | Depends on traffic |
| S3 | $5–10 | Storage + requests |
| API Gateway | $5–20 | $3.50 per million requests |
| Route 53 | $1–2 | Per hosted zone |
| Secrets Manager | $2–5 | $0.40 per secret/month |
| ECR | $5–10 | $0.10/GB storage |
| CloudWatch | $10–30 | Logs ingestion is the big cost |
| **Total** | **$315–420/month** | |

### Hidden Costs to Watch

- **NAT Gateway data processing**: $0.045/GB. If your services pull large images or transfer lots of data, this adds up. VPC endpoints for S3/ECR eliminate most of this.
- **Cross-AZ data transfer**: $0.01/GB between availability zones. Multi-AZ RDS and multi-AZ ECS both incur this.
- **CloudWatch Logs ingestion**: $0.50/GB. Verbose logging can cost more than the compute. Use log levels wisely.
- **Idle Fargate tasks**: unlike ouroboros' `scaleToZero`, Fargate services with `desired_count >= 1` run 24/7. Use Application Auto Scaling to scale to zero in non-prod.

### Compared to a Managed Platform

Nimbus (or similar PaaS) typically costs more per compute-hour but eliminates the ~40 hours/month of infrastructure management: VPC design, security patching, certificate rotation, monitoring setup, cost optimisation, IAM auditing. The question is not "which is cheaper" but "where do you want to spend your time."

---

## 6. What Nimbus Abstracts Away

The ouroboros module wraps ~17 AWS services into ~6 Python functions:

| AWS Component | ouroboros Equivalent | Code Reference |
|---------------|---------------------|----------------|
| ECR (image registry) | `create_or_update_image_version()` | `images.py:251` |
| Docker build | `deploy_image()` poll loop with `useCache: True` | `images.py:311` |
| ECS (container orchestration) | `create_or_update_service()` | `service.py:82` |
| ALB (load balancing) | Automatic per service | — |
| Auto-scaling | `minInstances`, `scaleToZero` | `service.py:120-125` |
| Instance sizing | `instanceTypeId: 52` / `26` | `service.py:108-116` |
| SSL/TLS | Automatic | — |
| DNS | Managed endpoint URLs | — |
| EventBridge (scheduling) | `create_or_update_workflow()` | `workflows.py:80` |
| Workflow image assignment | `step["imageId"] = workflow_image_id` | `workflows.py:138-139` |
| Image lifecycle | `delete_all_but_x_images(keep_last_n=3)` | `images.py:285` |
| Secrets | `build_args`, `build_secrets` | `service.py:157` |
| Networking (VPC, subnets) | Fully managed, invisible | — |
| IAM | Managed per tenant | — |
| Monitoring | Platform dashboards | — |

The value of Nimbus is not the infrastructure — it is the operational overhead you do not carry. Moving to raw AWS means owning every row in this table yourself: designing the VPC, rotating certificates, patching AMIs, auditing IAM, monitoring costs, responding to alarms at 3am.

---

## Prior Art

- **The Twelve-Factor App** — Factor V (Build, Release, Run) and Factor X (Dev/Prod Parity) are the philosophical foundation of this architecture
- **AWS Well-Architected Framework** — operational excellence, security, reliability, performance, cost optimisation, sustainability
- **Terraform Up & Running** (Yevgeniy Brikman) — the practical guide to Terraform in production, covers module design, state management, and team workflows
- **Production-Ready Microservices** (Susan Fowler) — operational maturity, stability, and reliability patterns for services
