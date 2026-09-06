import unittest
from unittest.mock import patch, MagicMock
from pathlib import Path
from database.connectors.duckdb import DuckDBConnector

class TestDuckDBConnector(unittest.TestCase):
    @patch('database.connectors.duckdb.duckdb')
    @patch('database.connectors.duckdb.Path')
    def test_init_creates_directory_and_connects(self, mock_path, mock_duckdb):
        # Setup mock path
        mock_home = MagicMock()
        mock_path.home.return_value = mock_home
        mock_duck_db_path = MagicMock()
        mock_home.__truediv__.return_value = mock_duck_db_path

        # Setup mock cursor
        mock_cursor = MagicMock()
        mock_connection = MagicMock()
        mock_connection.cursor.return_value.__enter__.return_value = mock_cursor
        mock_duckdb.connect.return_value = mock_connection

        # Assert directory was created
        mock_duck_db_path.mkdir.assert_called_once_with(exist_ok=True)

        # Assert connection was established
        mock_duckdb.connect.assert_called_once_with("duckdb:///duck_db_data/duck_db_data.duckdb")

        # Assert schema creation was attempted
        mock_cursor.execute.assert_called_once_with("CREATE SCHEMA IF NOT EXISTS TEST;")

    def test_database_url_property(self):
        with patch('database.connectors.duckdb.duckdb.connect'), \
             patch('database.connectors.duckdb.Path.home', return_value=Path('/mock_home')), \
             patch('database.connectors.duckdb.Path.mkdir'):
            connector = DuckDBConnector()
            self.assertEqual(connector.database_url, "duckdb:///duck_db_data/duck_db_data.duckdb")

    def test_connection_property(self):
        mock_connection = MagicMock()
        with patch('database.connectors.duckdb.duckdb.connect', return_value=mock_connection), \
             patch('database.connectors.duckdb.Path.home', return_value=Path('/mock_home')), \
             patch('database.connectors.duckdb.Path.mkdir'):
            connector = DuckDBConnector()
            self.assertEqual(connector.connection, mock_connection)

    def test_context_manager(self):
        mock_connection = MagicMock()
        with patch('database.connectors.duckdb.duckdb.connect', return_value=mock_connection), \
             patch('database.connectors.duckdb.Path.home', return_value=Path('/mock_home')), \
             patch('database.connectors.duckdb.Path.mkdir'):
            with DuckDBConnector() as connector:
                self.assertIsInstance(connector, DuckDBConnector)

            # Verify connection was closed
            mock_connection.close.assert_called_once()
