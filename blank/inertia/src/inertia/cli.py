from pathlib import Path
import copier
import typer
from rich.console import Console
from rich.prompt import Prompt

app = typer.Typer()

@app.command("templ")
def templ():
    """
    Create a new dbt project from a template.

    Examples:
        uv run inertia
    """
    console = Console()
    console.print("Welcome to Inertia!", style="bold blue")

    template_options = Path(__file__).parent / "templs"
    template_dirs = [d for d in template_options.iterdir() if d.is_dir()]

    template_names = [d.name for d in template_dirs]

    # Create a numbered list and prompt for selection
    console.print("\n[bold blue]Available templates:[/bold blue]")
    for idx, name in enumerate(template_names, 1):
        console.print(f"  {idx}. {name}")

    selection = Prompt.ask(
        "\n[bold blue]Select a template number[/bold blue]",
        choices=[str(i) for i in range(1, len(template_names) + 1)],
    )
    template_name = template_names[int(selection) - 1]

    src_path = template_options / template_name

    dist_path = Console().input("\n[bold blue]Enter the destination path (default: current directory): [/bold blue]")
    if dist_path:
        dst_path = Path.cwd() / dist_path
    else:
        dst_path = Path.cwd() / template_name

    dst_path.mkdir(parents=True, exist_ok=True)

    copier.run_copy(
        src_path=str(src_path),
        dst_path=str(dst_path.resolve()),
    )
