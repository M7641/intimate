# Vendoring a proto plugin

A vendored plugin is a small TOML manifest, committed under `proto-plugins/`,
that teaches proto how to download and install one CLI from its **official**
GitHub releases. It is referenced from `.prototools`:

```toml
[plugins]
ruff = "file://./proto-plugins/ruff.toml"
```

You only need one for tools proto does NOT have built in. The built-ins
(rust, python, node, bun, uv) never get a manifest.

## When to reuse vs author

- **Reuse**: the six manifests in `templates/proto-plugins/` (`ruff`, `ty`,
  `gitleaks`, `lefthook`, `sqruff`, `cargo-deny`) are tool-generic — copy them
  verbatim into any target that uses those tools. No edits needed.
- **Author a new one**: only when the target needs a non-built-in tool not in
  that set.

## Anatomy of a manifest

```toml
name = "Ruff"
type = "cli"

[resolve]
git-url = "https://github.com/astral-sh/ruff"   # where proto lists versions (tags)

[platform.linux]
download-file = "ruff-{arch}-unknown-linux-{libc}.tar.gz"
checksum-file = "ruff-{arch}-unknown-linux-{libc}.tar.gz.sha256"
archive-prefix = "ruff-{arch}-unknown-linux-{libc}"   # dir inside the archive to strip

[platform.macos]
download-file = "ruff-{arch}-apple-darwin.tar.gz"
# ...

[install]
download-url = "https://github.com/astral-sh/ruff/releases/download/{version}/{download_file}"
checksum-url = "https://github.com/.../{checksum_file}"

[install.arch]          # map proto arch names -> the tool's asset naming
x86 = "i686"
```

Tokens proto substitutes: `{version}`, `{arch}`, `{libc}`, `{download_file}`,
`{checksum_file}`.

## Authoring procedure

1. Open the tool's GitHub **Releases** page and look at one real release's asset
   names and the exact tag format.
2. Note the three things that vary between projects and trip people up:
   - **Tag prefix**: does the tag have a leading `v` (`v1.2.3`) or not
     (`1.2.3`)? `{version}` is the literal tag.
   - **Version in the filename?**: some assets embed the version
     (`cargo-deny-0.16.1-x86_64-...`), some don't (`ruff-x86_64-...`).
   - **Archive prefix / nesting**: does the tarball extract a binary at the root,
     or nested under an arch-named directory? Set `archive-prefix` to that dir.
3. Write `[platform.*]` for each OS the team uses (linux/macos/windows), mapping
   asset names with the tokens.
4. Add `[install.arch]` / `[install.libc]` maps if the tool's asset names use
   different arch/libc spellings than proto's.
5. Register it in `.prototools` under `[plugins]` and pin a version.
6. Test: `proto install <tool>` then `<tool> --version`.

## Real quirks captured in the templates

These are documented in the template comments — they're the kind of thing that
costs an hour each:

- **ruff** (cargo-dist): tags have **no `v` prefix** and the asset filename
  **omits the version**. The community manifest got this wrong; the vendored one
  is corrected.
- **ty** (cargo-dist, same scheme as ruff): no `v` prefix, no version in
  filename, binary nested under an arch-named dir. No community plugin exists —
  authored here.
- **cargo-deny**: no `v` tag prefix, version **is** in the filename, binary
  nested under an arch-named dir.
- **sqruff**: macOS assets are named `sqruff-darwin-{arch}.tar.gz`.

When in doubt, mirror the closest existing template and diff against the tool's
actual release assets.
