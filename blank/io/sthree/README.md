# Sthree

```bash
uv add git+https://github.com/nimbus-labs/nimbus-monorepo.git@main#subdirectory=blank/io/sthree
```

An interesting issue came up for me when adding this one. UV's lock file will lock to a commit hash. Then it will not change until you `uv lock --upgrade`. This will then pull the latest commit on that branch you are targeting it towards.

It makes perfect sense, I just had a bit of ignorance on how that lock file totally locks in a specific commit rather than just the address. Learning moment.
