"""Unit + property tests for the config layer: override precedence, the nested
TOML -> flat RunConfig mapping, and the guarantee that a generated starter
config parses back (template -> load_config round-trip)."""

from __future__ import annotations

from pathlib import Path

from hypothesis import given
from hypothesis import strategies as st

from gauge.config import RunConfig, load_config, template

# --- apply_overrides: CLI flag > file > default -----------------------------


def test_apply_overrides_skips_none_and_empty_list():
    cfg = RunConfig(image="from-file", run_args=["-e", "A=1"])
    cfg.apply_overrides(image=None, run_args=[])  # both "not provided"
    assert cfg.image == "from-file"
    assert cfg.run_args == ["-e", "A=1"]


def test_apply_overrides_sets_provided_values():
    cfg = RunConfig()
    cfg.apply_overrides(image="cli", port=9000, run_args=["-e", "B=2"])
    assert cfg.image == "cli"
    assert cfg.port == 9000
    assert cfg.run_args == ["-e", "B=2"]


def test_apply_overrides_applies_explicit_zero():
    # 0 is falsy but *provided* — it must win over the default, unlike None.
    cfg = RunConfig(requests=2000)
    cfg.apply_overrides(requests=0)
    assert cfg.requests == 0


def test_apply_overrides_returns_self_for_chaining():
    cfg = RunConfig()
    assert cfg.apply_overrides(port=1234) is cfg


# --- derived accessors ------------------------------------------------------


def test_effective_host_port_defaults_to_port():
    assert RunConfig(port=8000).effective_host_port == 8000


def test_effective_host_port_prefers_explicit_host_port():
    assert RunConfig(port=8000, host_port=9000).effective_host_port == 9000


def test_base_url_uses_effective_host_port():
    assert RunConfig(port=8000, host_port=9000).base_url() == "http://127.0.0.1:9000"


# --- load_config: nested TOML -> flat RunConfig -----------------------------


def test_load_config_maps_nested_sections(tmp_path: Path):
    toml = tmp_path / "app.toml"
    toml.write_text(
        """
        image = "svc:latest"
        max_duration = 12.0

        [build]
        context = "."
        dockerfile = "Dockerfile.prod"

        [container]
        port = 3000
        host_port = 3001
        memory = "512m"
        run_args = ["-e", "K=v"]

        [ready]
        url = "http://127.0.0.1:3000/health"
        timeout = 5

        [load]
        openapi_url = "http://127.0.0.1:3000/openapi.json"
        requests = 500
        concurrency = 8

        [output]
        dir = "report"
        interval = 0.25
        """
    )
    cfg = load_config(toml)
    assert cfg.image == "svc:latest"
    assert cfg.max_duration == 12.0
    assert cfg.build_context == Path(".")
    assert cfg.dockerfile == Path("Dockerfile.prod")
    assert cfg.port == 3000 and cfg.host_port == 3001
    assert cfg.memory == "512m"
    assert cfg.run_args == ["-e", "K=v"]
    assert cfg.ready_url == "http://127.0.0.1:3000/health"
    assert cfg.ready_timeout == 5
    assert cfg.openapi_url.endswith("/openapi.json")
    assert cfg.requests == 500 and cfg.concurrency == 8
    assert cfg.out_dir == Path("report")
    assert cfg.interval == 0.25


def test_load_config_applies_defaults_for_missing_sections(tmp_path: Path):
    toml = tmp_path / "min.toml"
    toml.write_text('image = "svc"\n')
    cfg = load_config(toml)
    assert cfg.image == "svc"
    assert cfg.port == 8080  # container default
    assert cfg.ready_timeout == 30.0
    assert cfg.requests == 2000 and cfg.concurrency == 16
    assert cfg.interval == 0.5
    assert cfg.build_context is None and cfg.out_dir is None


# --- template -> load_config round-trip -------------------------------------


def _roundtrip(tmp_path: Path, **kwargs) -> RunConfig:
    written = tmp_path / "gauge.toml"
    written.write_text(template(**kwargs))
    return load_config(written)


def test_template_single_url_roundtrips(tmp_path: Path):
    cfg = _roundtrip(
        tmp_path, image="my-app", port=8000, build_context=None, openapi=False
    )
    assert cfg.image == "my-app"
    assert cfg.port == 8000
    assert cfg.load_url == "http://127.0.0.1:8000/"  # active line
    assert cfg.openapi_url is None  # commented out
    assert cfg.ready_url == "http://127.0.0.1:8000/"


def test_template_openapi_roundtrips(tmp_path: Path):
    cfg = _roundtrip(tmp_path, image="api", port=9000, build_context=None, openapi=True)
    assert cfg.openapi_url == "http://127.0.0.1:9000/openapi.json"
    assert cfg.load_url is None
    assert cfg.requests == 2000 and cfg.concurrency == 16


def test_template_with_build_context_uncomments_build(tmp_path: Path):
    cfg = _roundtrip(tmp_path, image="app", port=8080, build_context=".", openapi=False)
    assert cfg.build_context == Path(".")


@given(
    image=st.from_regex(r"[a-z][a-z0-9._/-]{0,20}", fullmatch=True),
    port=st.integers(min_value=1, max_value=65535),
    openapi=st.booleans(),
)
def test_template_always_parses_and_preserves_image_port(image, port, openapi):
    # Whatever init emits must be valid TOML that load_config accepts, with the
    # image + port carried through untouched. tmp_path fixtures don't compose with
    # @given, so parse from an in-memory temp file.
    import tempfile

    with tempfile.NamedTemporaryFile("w", suffix=".toml", delete=False) as fh:
        fh.write(template(image=image, port=port, build_context=None, openapi=openapi))
        path = Path(fh.name)
    cfg = load_config(path)
    path.unlink()
    assert cfg.image == image
    assert cfg.port == port
