# Production Readiness Standards

A complete reference for every aspect that contributes to a production-grade service. Each document answers three questions:

1. **What** is this practice?
2. **Why** does it matter — what goes wrong without it?
3. **How** is it implemented (or planned) in this codebase?

---

## How to Use This Guide

- Each file is self-contained — read only what is relevant to your current task.
- **Status** badges indicate whether the aspect is already implemented or planned.
- Code examples reference actual files in this repository.

---

## Observability

Understanding what is happening inside a running system.

| #   | Aspect                   | File                                                                                 | Status                      |
| --- | ------------------------ | ------------------------------------------------------------------------------------ | --------------------------- |
| 1   | Structured Logging       | [observability/structured-logging.md](observability/structured-logging.md)           | Implemented                 |
| 2   | Request Correlation      | [observability/request-correlation.md](observability/request-correlation.md)         | Implemented                 |
| 3   | Metrics & the RED Method | [observability/metrics-and-red-method.md](observability/metrics-and-red-method.md)   | Implemented                 |
| 4   | Distributed Tracing      | [observability/distributed-tracing.md](observability/distributed-tracing.md)         | Implemented (feature-gated) |
| 5   | Alerting & Dashboards    | [observability/alerting-and-dashboards.md](observability/alerting-and-dashboards.md) | Reference                   |

## Reliability

Keeping the service available and recoverable under adverse conditions.

| #   | Aspect             | File                                                                   | Status      |
| --- | ------------------ | ---------------------------------------------------------------------- | ----------- |
| 6   | Health Checks      | [reliability/health-checks.md](reliability/health-checks.md)           | Implemented |
| 7   | Graceful Shutdown  | [reliability/graceful-shutdown.md](reliability/graceful-shutdown.md)   | Implemented |
| 8   | Panic Recovery     | [reliability/panic-recovery.md](reliability/panic-recovery.md)         | Implemented |
| 9   | Connection Pooling | [reliability/connection-pooling.md](reliability/connection-pooling.md) | Implemented |
| 10  | Timeouts           | [reliability/timeouts.md](reliability/timeouts.md)                     | Implemented |
| 11  | Circuit Breakers   | [reliability/circuit-breakers.md](reliability/circuit-breakers.md)     | Implemented |
| 12  | Rate Limiting      | [reliability/rate-limiting.md](reliability/rate-limiting.md)           | Implemented |
| 12b | Request Body Limits | [reliability/request-body-limits.md](reliability/request-body-limits.md) | Implemented |

## Correctness

Ensuring the service behaves predictably and communicates failures clearly.

| #   | Aspect            | File                                                                 | Status      |
| --- | ----------------- | -------------------------------------------------------------------- | ----------- |
| 13  | Error Taxonomy    | [correctness/error-taxonomy.md](correctness/error-taxonomy.md)       | Implemented |
| 14  | Input Validation  | [correctness/input-validation.md](correctness/input-validation.md)   | Implemented |
| 15  | API Documentation | [correctness/api-documentation.md](correctness/api-documentation.md) | Implemented |
| 15b | Parameter Validation | [correctness/parameter-validation.md](correctness/parameter-validation.md) | Implemented |

## Security

Protecting the service and its data from misuse.

| #   | Aspect                  | File                                                                 | Status      |
| --- | ----------------------- | -------------------------------------------------------------------- | ----------- |
| 16  | Injection Prevention    | [security/injection-prevention.md](security/injection-prevention.md) | Implemented |
| 17  | CORS & Security Headers | [security/cors-and-headers.md](security/cors-and-headers.md)         | Implemented |
| 18  | Secret Management       | [security/secret-management.md](security/secret-management.md)       | Partial     |
| 17b | Authentication          | [deployment/authentication.md](deployment/authentication.md)         | Reference   |

## Deployment

Guides for deploying the service to production environments.

| # | Aspect | File | Status |
|---|--------|------|--------|
| 23 | Authentication | [deployment/authentication.md](deployment/authentication.md) | Reference |
| 24 | Containerization | [deployment/container.md](deployment/container.md) | Reference |

## Performance

Measuring and optimising how fast the service responds.

| #   | Aspect               | File                                                                       | Status      |
| --- | -------------------- | -------------------------------------------------------------------------- | ----------- |
| 19  | Slow Query Detection | [performance/slow-query-detection.md](performance/slow-query-detection.md) | Implemented |
| 20  | Response Compression | [performance/response-compression.md](performance/response-compression.md) | Implemented |
| 21  | Load Testing         | [performance/load-testing.md](performance/load-testing.md)                 | Planned     |
| 22  | Middleware Ordering  | [performance/middleware-ordering.md](performance/middleware-ordering.md)   | Implemented |

## Retrospectives

Lessons captured from real debugging sessions.

| #   | Aspect                      | File                                                                         | Status        |
| --- | --------------------------- | ---------------------------------------------------------------------------- | ------------- |
| 25  | Data Explorer Stabilization | [frontend-stabilization-postmortem.md](frontend-stabilization-postmortem.md) | Retrospective |
