import subprocess
import typer

from ouroboros.service import deploy_service

app = typer.Typer()

@app.command(
    context_settings={"allow_extra_args": True, "ignore_unknown_options": True},
)
def vrp(ctx: typer.Context) -> None:
    """
    Run vehicle routing problem commands with the correct environment settings.

    https://github.com/reinterpretcat/vrp/tree/master?tab=readme-ov-file
    https://doc.rust-lang.org/cargo/commands/cargo-install.html
    https://reinterpretcat.github.io/vrp/getting-started/import.html

    https://github.com/Project-OSRM/osrm-backend?tab=readme-ov-file is a C++ option.

    curl https://sh.rustup.rs -sSf | sh
    cargo install vrp-cli

    vrp-cli solve pragmatic problem.json -m routing_matrix.json -o solution.json --search-mode=deep --geo-json=solution.geojson
    """

    try:
        subprocess.run(["which", "vrp-cli"], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except subprocess.CalledProcessError:
        typer.echo("Error: vrp-cli is not installed")
        typer.echo("To install, run: cargo install vrp-cli")
        raise typer.Exit(1)

    args = list(ctx.args)
    subprocess.run(["vrp-cli", *args], check=True)

@app.command("deploy-osrm")
def deploy_osrm_service() -> None:
    """
    Deploy the OSRM service using Ouroboros.

    Examples:
        uv run destiny deploy-osrm
    """
    deploy_service(
        service_name="osrm-instance",
        service_type="api",
        target="prod",
        dockerfile_path="Dockerfile.osrm",
        version="v1",
        artifact={
            "path": "Dockerfile.osrm",
            "ignore_files": [".dockerignore"],
        },
    )

if __name__ == "__main__":
    app()
