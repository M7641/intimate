# Inertia

Inertia is a set of templates.

The templates are rendered through [Copier](https://copier.readthedocs.io/en/stable/).

## Usage

### Run without installing (recommended)

`uvx` will fetch, build, and run inertia in an ephemeral environment:

```bash
uvx --from "git+https://github.com/nimbus-labs/nimbus-monorepo.git@main#subdirectory=blank/inertia" inertia templ
```

The `--from` flag is required because the package name (`inertia`) does not match the git URL — it lives in a subdirectory of the `intimate` repo.

### Install persistently as a uv tool

If you'd rather have `inertia` on your `$PATH` permanently:

```bash
uv tool install "git+https://github.com/nimbus-labs/nimbus-monorepo.git@main#subdirectory=blank/inertia"
inertia templ
```

Upgrade later with `uv tool upgrade inertia`, or remove with `uv tool uninstall inertia`.

### Install as a project dependency

```bash
uv add "git+https://github.com/nimbus-labs/nimbus-monorepo.git@main#subdirectory=blank/inertia"
```

or you can add it directly to your `pyproject.toml`:

```toml
[project]
dependencies = [
    "inertia @ git+https://github.com/nimbus-labs/nimbus-monorepo.git#egg=inertia&subdirectory=blank/inertia@main",
]
```

Then run the inertia command to create a new inertia project:

```bash
uv run inertia templ
```

Copier will then prompt you for some information about your new project and generate the files for you.

## Templates

The inertia templates include:

1. react
2. dbt
3. maturin
4. starlight

dbt and maturin are very similar to their traditional setups and are just for convenience. Starlight and React are two applications. Starlight is for github pages apps and is good for documentation sites. React is a react app, but has a lot of boilerplate for Nimbus. This includes a dockerfile, a FastAPI backend, and a lot of other stuff.

Further documentation for each template can be found in their respective folders.
