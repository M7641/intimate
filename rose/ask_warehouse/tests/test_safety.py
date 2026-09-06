"""Safety validation must reject every dangerous SQL shape and accept benign ones."""

import pytest

from common_py.ask_warehouse.safety import (
    UnsafeSqlError,
    parse_llm_output,
    validate_and_prepare,
)


class TestRejectsDangerousSql:
    @pytest.mark.parametrize(
        "sql",
        [
            "DELETE FROM sales_product_week",
            "UPDATE sales_product_week SET sales_units = 0",
            "INSERT INTO sales_product_week (sales_units) VALUES (1)",
            "DROP TABLE sales_product_week",
            "CREATE TABLE x (a INT)",
            "ALTER TABLE sales_product_week ADD COLUMN x INT",
            "TRUNCATE TABLE sales_product_week",
            "GRANT SELECT ON sales_product_week TO public",
        ],
    )
    def test_root_verbs_blocked(self, sql: str) -> None:
        with pytest.raises(UnsafeSqlError):
            validate_and_prepare(sql)

    def test_multi_statement_blocked(self) -> None:
        with pytest.raises(UnsafeSqlError, match="multi-statement"):
            validate_and_prepare("SELECT 1; DROP TABLE sales_product_week")

    def test_unparseable_blocked(self) -> None:
        with pytest.raises(UnsafeSqlError, match="unparseable"):
            validate_and_prepare("SELECT FROM WHERE")

    def test_empty_blocked(self) -> None:
        with pytest.raises(UnsafeSqlError):
            validate_and_prepare("")

    def test_non_allowlisted_table_blocked(self) -> None:
        with pytest.raises(UnsafeSqlError, match="outside the curated allowlist"):
            validate_and_prepare("SELECT * FROM definitely_not_a_real_table LIMIT 1")


class TestAcceptsBenignSql:
    def test_simple_select_passes(self) -> None:
        result = validate_and_prepare("SELECT * FROM sales_product_week LIMIT 10")
        assert "sales_product_week" in result.lower()

    def test_cte_with_select_passes(self) -> None:
        sql = (
            "WITH x AS (SELECT date_week FROM sales_product_week) "
            "SELECT * FROM x LIMIT 1"
        )
        result = validate_and_prepare(sql)
        assert result  # should not raise

    def test_select_with_join_passes(self) -> None:
        sql = (
            "SELECT s.date_week, l.lfl "
            "FROM sales_product_week s JOIN dim__lfl_stores l USING (store_id) "
            "LIMIT 10"
        )
        validate_and_prepare(sql)  # should not raise

    def test_select_without_table_passes(self) -> None:
        # ``SELECT 1`` references no tables — allowlist check is moot.
        validate_and_prepare("SELECT 1 LIMIT 1")

    def test_trailing_semicolon_tolerated(self) -> None:
        validate_and_prepare("SELECT * FROM sales_product_week LIMIT 10;")


class TestLimitInjection:
    def test_missing_limit_gets_default(self) -> None:
        result = validate_and_prepare("SELECT * FROM sales_product_week")
        assert "LIMIT 10000" in result

    def test_existing_limit_preserved(self) -> None:
        result = validate_and_prepare("SELECT * FROM sales_product_week LIMIT 42")
        assert "LIMIT 42" in result
        assert "LIMIT 10000" not in result

    def test_cte_without_limit_gets_default(self) -> None:
        sql = (
            "WITH x AS (SELECT date_week FROM sales_product_week LIMIT 5) "
            "SELECT * FROM x"
        )
        result = validate_and_prepare(sql)
        assert "LIMIT 10000" in result


class TestParseLlmOutput:
    def test_extracts_sql_from_fence(self) -> None:
        from common_py.ask_warehouse.safety import ParsedSQL

        raw = "Here you go:\n```sql\nSELECT 1\n```"
        result = parse_llm_output(raw)
        assert isinstance(result, ParsedSQL)
        assert result.sql == "SELECT 1"

    def test_extracts_first_sql_fence_when_multiple(self) -> None:
        from common_py.ask_warehouse.safety import ParsedSQL

        raw = "```sql\nSELECT 1\n```\n```sql\nSELECT 2\n```"
        result = parse_llm_output(raw)
        assert isinstance(result, ParsedSQL)
        assert result.sql == "SELECT 1"

    def test_unsure_returns_unsure_with_reason(self) -> None:
        from common_py.ask_warehouse.safety import ParsedUnsure

        raw = "-- UNSURE: I do not know which table holds planogram data"
        result = parse_llm_output(raw)
        assert isinstance(result, ParsedUnsure)
        assert "planogram" in result.reason

    def test_no_fence_returns_plain_answer(self) -> None:
        from common_py.ask_warehouse.safety import ParsedAnswer

        raw = "Hi! I can answer questions about sales, stock, and stores."
        result = parse_llm_output(raw)
        assert isinstance(result, ParsedAnswer)
        assert result.text == raw

    def test_unterminated_fence_raises(self) -> None:
        with pytest.raises(UnsafeSqlError):
            parse_llm_output("```sql\nSELECT 1")
