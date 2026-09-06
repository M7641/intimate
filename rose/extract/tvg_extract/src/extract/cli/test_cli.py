"""CLI smoke tests for the model-free commands (domains, describe)."""

from __future__ import annotations

from typer.testing import CliRunner

from extract.cli import app

runner = CliRunner()


def test_domains_lists_registered_domains():
    result = runner.invoke(app, ["domains"])
    assert result.exit_code == 0
    assert "clothing" in result.stdout
    assert "electronics" in result.stdout


def test_describe_prints_the_prompt_contract():
    result = runner.invoke(app, ["describe", "clothing"])
    assert result.exit_code == 0
    assert "dress" in result.stdout
