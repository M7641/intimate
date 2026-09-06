import typer
from rich.console import Console
from typing_extensions import Annotated

console = Console()
app = typer.Typer()

from .expectations import OutputExpectations


def build_pretty_output(results: dict) -> str:
    try:
        build_output = f"Column: {results['expectation_config']['kwargs']['column']}"
    except KeyError:
        build_output = "Column: Custom Expectation"

    build_output += f" | Expectation: {results['expectation_config']['type']}"
    build_output += f" | Result: {results['success']}"
    build_output += f" | Value: {results['result']['observed_value']}"
    return build_output


@app.command()
def check_outputs(
    campaign_id: Annotated[str, "Campaign UUID to validate (e.g. XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX)"] = "",
) -> None:
    console.print("𝓒𝓱𝓮𝓬𝓴 𝓞𝓾𝓽𝓹𝓾𝓽𝓼", style="bold green")

    if campaign_id == "":
        console.print("Validating all campaign IDs", style="bold green")
    else:
        console.print(f"Validating campaign ID: {campaign_id}", style="bold green")

    console.print("=====================", style="bold green")
    expectations = OutputExpectations()
    expectations.add_positive_forecast_expectation()
    expectations.add_negative_elasticity_expectation()
    expectations.add_maximum_discount_expectation()
    expectations.add_minimum_discount_expectation()
    results = expectations.validate(campaign_id=campaign_id)

    for i in results["results"]:
        build_output = build_pretty_output(i)
        sucess_color = "green" if i["success"] else "red"
        console.print(build_output, style=f"bold {sucess_color}")


    console.print("=====================", style="bold green")
    console.print("𝓒𝓱𝓮𝓬𝓴 𝓒𝓸𝓶𝓹𝓵𝓮𝓽𝓮", style="bold green")


if __name__ == "__main__":
    app()
