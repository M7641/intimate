import typer

app = typer.Typer()
examples_app = typer.Typer(
    help="Runnable examples — each generates synthetic data, trains, and prints results."
)
app.add_typer(examples_app, name="examples")


@examples_app.command("deep-sets")
def deep_sets() -> None:
    """Train a Deep Sets encoder on synthetic clustered data."""
    from mana.examples.deep_sets import main

    main()


@examples_app.command("set-transformer")
def set_transformer() -> None:
    """Train a Set Transformer encoder on synthetic clustered data."""
    from mana.examples.set_transformer import main

    main()


@examples_app.command("polars-tabular")
def polars_tabular() -> None:
    """Train on a Polars DataFrame with mixed categorical + continuous features."""
    from mana.examples.polars_tabular import main

    main()


@examples_app.command("embedding-regression")
def embedding_regression() -> None:
    """End-to-end: data → mimic embeddings → regression prediction."""
    from mana.examples.embedding_regression import main

    main()


@examples_app.command("retrieval-recommender")
def retrieval_recommender() -> None:
    """Train a retrieval recommender on synthetic pre-computed embeddings."""
    from mana.examples.retrieval_recommender import main

    main()


@examples_app.command("ranking-recommender")
def ranking_recommender() -> None:
    """Train a ranking recommender on synthetic pre-computed embeddings."""
    from mana.examples.ranking_recommender import main

    main()


if __name__ == "__main__":
    app()
