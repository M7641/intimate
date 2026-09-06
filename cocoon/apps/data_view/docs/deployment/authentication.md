# Authentication & Authorization

> Status: **Architectural Decision** — authentication is delegated to the infrastructure layer

## What

This service does **not implement authentication or authorization internally**. It relies on an upstream reverse proxy or API gateway to enforce identity verification before requests reach the application.

## Why

| Approach                                 | Trade-off                                                                                                      |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| In-app auth (JWT, session)               | Tightly couples auth logic to the data service; every endpoint needs middleware; token validation adds latency |
| Gateway auth (Nginx, Envoy, API Gateway) | Single enforcement point; service stays focused on data; auth can be swapped without code changes              |

For internal tooling that serves a known set of users behind a VPN or SSO gateway, delegating auth to infrastructure is the standard pattern.

## How It Works

```
User → SSO/IdP → API Gateway → data_view service
                  │
                  ├── Validates token (JWT, OAuth2, SAML)
                  ├── Sets X-Auth-Email header
                  └── Forwards only authenticated requests
```

The service reads the authenticated user identity from the `X-Auth-Email` header (set by the gateway). It does not validate this header — it trusts the gateway.

## Deployment Requirements

1. **Never expose this service directly to the internet** — always place it behind an authenticated gateway
2. **Configure the gateway** to set `X-Auth-Email` from the validated identity
3. **Strip `X-Auth-Email`** on ingress to prevent clients from spoofing it

## Future Considerations

If the service needs to enforce fine-grained authorization (e.g., schema-level access control), add middleware that reads `X-Auth-Email` and checks permissions against an allow-list or policy engine.
