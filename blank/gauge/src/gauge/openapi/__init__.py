"""Derive concrete GET URLs to hammer from an app's OpenAPI spec."""

from .openapi import derive_get_urls, fetch_spec

__all__ = ["derive_get_urls", "fetch_spec"]
