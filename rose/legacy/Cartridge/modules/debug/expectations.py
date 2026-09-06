import os

import great_expectations as gx


class ExpectMaximumGaurdRail(gx.expectations.UnexpectedRowsExpectation):

    """Expectation for the maximum markdown discount."""

    unexpected_rows_query: str = """
    SELECT 1 - (markdown_price / current_price) as discount
    FROM {batch}
    WHERE discount > 0.8
    """
    description: str = "Maximum markdown discount is 80%"
    result_format: str = "COMPLETE"


class ExpectMinimumGaurdRail(gx.expectations.UnexpectedRowsExpectation):

        """Expectation for the minimum markdown discount."""

        unexpected_rows_query: str = """
        SELECT 1 - (markdown_price / current_price) as discount
        FROM {batch}
        WHERE discount <= 0.0
        """
        description: str = "Minimum markdown discount is 0%"
        result_format: str = "COMPLETE"


class OutputExpectations:

    """Class to define output expectations for Markdown Output Table."""

    def __init__(self) -> None:
        """Initialize the OutputExpectations class."""
        self.context = gx.get_context()
        self.expectations = gx.ExpectationSuite(
            name="output_expectations",
        )
        self.data_source = self.context.data_sources.add_snowflake(
            name="nimbus_pricing",
            account=os.getenv("SNOWFLAKE_ACCOUNT"),
            user=os.getenv("SNOWFLAKE_USERNAME"),
            password=os.getenv("SNOWFLAKE_PASSWORD"),
            database=os.getenv("SNOWFLAKE_DATABASE"),
            schema="TRANSFORM",
            warehouse=os.getenv("SNOWFLAKE_WAREHOUSE"),
            role=os.getenv("SNOWFLAKE_ROLE"),
        )
        self.table_asset = self.data_source.add_table_asset(
            table_name="pricing__markdown_campaign_product_solutions",
            name="pricing__markdown_campaign_product_solutions",
        )
        self.campaign_id_batch_definition = self.table_asset.add_batch_definition(
            name="campaign_id_and_version",
            partitioner={
                "column_names": ["campaign_id", "campaign_version"],
            },
        )

    def validate(self, campaign_id: str, campaign_version: int = 1) -> dict:
        """Validate the output expectations."""
        if campaign_id == "":
            return self.campaign_id_batch_definition.get_batch().validate(
                self.expectations,
            )

        return self.campaign_id_batch_definition.get_batch(
            batch_parameters={
                "campaign_id": campaign_id,
                "campaign_version": campaign_version,
            },
        ).validate(self.expectations)

    def add_positive_forecast_expectation(self) -> None:
        """Add an expectation that the forecasted units sold is positive."""
        expectation = gx.expectations.ExpectColumnMinToBeBetween(
            column="predicted_units_sold",
            min_value=0,
            max_value=None,
            result_format="COMPLETE",
        )
        self.expectations.add_expectation(expectation)

    def add_negative_elasticity_expectation(self) -> None:
        """Add an expectation that the elasticity is negative."""
        expectation = gx.expectations.ExpectColumnMaxToBeBetween(
            column="elasticity",
            min_value=None,
            max_value=0,
            result_format="COMPLETE",
        )
        self.expectations.add_expectation(expectation)

    def add_maximum_discount_expectation(self) -> None:
        """Add an expectation that the maximum markdown discount is 80%."""
        expectation = ExpectMaximumGaurdRail()
        self.expectations.add_expectation(expectation)

    def add_minimum_discount_expectation(self) -> None:
        """Add an expectation that the minimum markdown discount is 0%."""
        expectation = ExpectMinimumGaurdRail()
        self.expectations.add_expectation(expectation)
