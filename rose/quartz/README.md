# Quartz — SolidJS pilot

A pilot web application that mirrors the React/FastAPI template
(`blank/inertia/.../templs/react`) but swaps the frontend for the **SolidJS**
ecosystem:

- **[SolidJS](https://www.solidjs.com/)** — fine-grained reactive UI
- **[shadcn-solid](https://shadcn-solid.com/)** — shadcn/ui components ported to Solid (built on [Kobalte](https://kobalte.dev/))
- **[TanStack Solid Router](https://tanstack.com/router)** — type-safe client routing
- **[TanStack Solid Query](https://tanstack.com/query)** — client-side data caching
- **[Tailwind CSS v4](https://tailwindcss.com/)** — styling

The compiled SPA is served in production by a **FastAPI** instance, using the
exact same serving strategy as the React template (static mount + a catch-all
route that returns `index.html` so the client router can resolve 404s).

## How serving works

`vite build` emits the SPA into `frontend/dist`. `quartz.api.api` then:

1. mounts `frontend/dist` as static files, and
2. defines a catch-all `GET /{full_path:path}` that returns `index.html` for any
   non-API, non-file path — letting TanStack Router own navigation.

`GET /api/*` paths are reserved for the backend (the catch-all 404s them so they
never accidentally return the SPA shell). There is one example endpoint,
`GET /api/hello`, that the home page fetches through TanStack Query.

## CLI

The Typer CLI orchestrates `bun` (frontend) and `uvicorn` (backend):

```zsh
uv run quartz --help

uv run quartz install   # bun install
uv run quartz dev       # vite dev server (hot reload) on :5173
uv run quartz build     # bun install --frozen-lockfile + vite build -> frontend/dist
uv run quartz start     # uvicorn on :8050 (builds first on non-macOS)
```

In local dev you typically run two processes: `quartz dev` (Vite on `:5173`,
proxying `/api` to `:8050`) and `quartz start` (FastAPI on `:8050`). On
non-macOS hosts, `start` builds the frontend first and serves everything from
`:8050` with multiple uvicorn workers.

## Frontend layout

```
frontend/
  index.html                 # mounts #root, loads /src/main.tsx
  vite.config.ts             # vite-plugin-solid + tailwind + tsconfig paths
  components.json            # shadcn-solid CLI config
  src/
    main.tsx                 # render() + code-based TanStack Router tree
    pages/home.tsx           # demo page (Card + Button + useQuery)
    components/
      ui/button.tsx          # shadcn-solid Button (Kobalte polymorphic)
      ui/card.tsx            # shadcn-solid Card
      themeProvider.tsx      # signal-based light/dark/cyber theme context
      themeToggle.tsx        # cycles the theme
      Header.tsx
    assets/
      index.css              # tailwind + theme imports + shadcn tokens
      themes/                # light / dark / cyber CSS variable sets
    lib/utils.ts             # cn() helper
```

Routing is defined **code-first in `main.tsx`** (not file-based), matching the
React template's convention. To add a route, create a `createRoute({...})` and
append it to `RootRoute.addChildren([...])`.

## Adding shadcn-solid components

From inside `frontend/`:

```zsh
bunx shadcn-solid@latest add <component-name>
```

Components land in `src/components/ui` and use the `cn()` helper from
`src/lib/utils.ts`.

## Notes vs. the React template

- The backend keeps the same middleware stack (profiling, process-time headers,
  session logging, proxy-path rewriting, thread accounting), gzip, Prometheus
  metrics and the APScheduler background job.
- The Redshift connection in the React template's `lifespan` and the DB insert
  in `log_message` were dropped so the pilot runs standalone — `log_message`
  now emits a structured log line. Re-wire `database.composites.insert_data`
  there when you want persistence.
