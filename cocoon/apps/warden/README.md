# warden

The cocoon **control layer**. `warden` deploys the *other* cocoon apps
(`data_view`, `redhouse`, `snowhouse`, …) into the Nimbus tenant it is run
against.

## The idea: two layers, two credentials

Cocoon splits in two.

- The **application layer** — `snowhouse`, `redhouse`, `data_view` — is
  tenant- and credential-specific. Each service needs warehouse credentials
  (Snowflake, Redshift, …) to read data.
- The **control layer** — `warden` — is deliberately relaxed. Its only
  credential is the tenant's Nimbus `API_KEY`: a *deploy* token, not a data one.

In Nimbus there is no explicit tenant id — **the `API_KEY` _is_ the tenant**.
`ouroboros::Client::from_env()` reads it, and every deploy is scoped to it. So
"deploy the apps into the tenant `warden` runs in" is simply: run `warden` with
that tenant's `API_KEY` in the environment. `warden` reads that one token and
nothing else.

## Single source of truth

`warden` never hard-codes how an app deploys. Every app's deploy shape — image
name, Dockerfile, build args, credentials — lives once in the
[`deploy-catalog`](../../crates/deploy-catalog) crate, read by **both** `warden`
and each app's own `deploy` subcommand. They cannot drift.

## Commands

```bash
warden list [--live]        # the deployable apps (and, with --live, which are deployed)
warden plan <app|--all>     # show what a deploy would do — no API_KEY, no network
warden deploy <app|--all>   # build + deploy into the current tenant (needs API_KEY)
```

```bash
export API_KEY="…"          # the target tenant's deploy token
warden deploy snowhouse     # deploy one app into that tenant
warden deploy --all         # deploy every catalogue app
```

## Credentials: by reference

`warden` deploys **by reference**: it reads no warehouse credential. For an app
that needs data credentials it does not inject them — it reports which secret
names the tenant must provide to the running service:

```
! by reference: the tenant must provide [REDSHIFT_USERNAME, REDSHIFT_PASSWORD] to the running service
```

This is what keeps the control layer relaxed: a data credential never passes
through `warden`.

### The transitional path — `--from-env`

Today each app's Dockerfile still reads its credentials from **build args** and
bakes them into the image (a compromise the Dockerfiles note explicitly). The
per-app `deploy` subcommands use this path — the catalogue's `SecretMode::FromEnv`
— unchanged. `warden deploy --from-env` opts into the same behaviour: it reads
the credentials from *its* environment and forwards them. Use it only when the
running services cannot yet get their credentials from the tenant directly — and
know that in this mode `warden` does hold the data credentials.

### The remaining step

True by-reference means the running service gets its credentials from the
tenant's own secret store as **runtime** environment, so nothing is baked and
`warden` never sees a value. That needs `ouroboros` to set service-level
environment from Nimbus secret references (it currently sets only image, resources,
and tags). The catalogue already declares credentials as references
(`runtime_secrets`), so only the resolution step changes — the design is not
blocked by the app specs.
