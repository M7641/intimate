"""Scraper parsing on static HTML — no network. Network wrappers are untested."""

from __future__ import annotations

import pytest

# Skipped cleanly when the optional `scrape` extra is not installed.
pytest.importorskip("bs4")

from extract.scrape.scrape import Product, parse_product  # noqa: E402

BASE = "https://shop.example.com/p/1"


def test_parses_jsonld_product():
    html = """
    <html><head>
      <script type="application/ld+json">
      {"@type": "Product", "name": "Acme Maxi Dress",
       "description": "A flowing red maxi dress.",
       "image": "https://cdn.example.com/dress.jpg"}
      </script>
    </head><body></body></html>
    """
    product = parse_product(html, BASE)
    assert product.title == "Acme Maxi Dress"
    assert "maxi dress" in product.description.lower()
    assert product.image_url == "https://cdn.example.com/dress.jpg"


def test_falls_back_to_open_graph_and_resolves_relative_image():
    html = """
    <html><head>
      <meta property="og:title" content="OG Trainers">
      <meta property="og:description" content="Blue running trainers.">
      <meta property="og:image" content="/img/trainers.jpg">
    </head><body></body></html>
    """
    product = parse_product(html, BASE)
    assert product.title == "OG Trainers"
    assert product.description == "Blue running trainers."
    assert product.image_url == "https://shop.example.com/img/trainers.jpg"


def test_image_fallback_prefers_alt_match_over_logo():
    # No JSON-LD/og:image: pick the <img> whose alt matches the title, not the
    # logo declared first.
    html = """
    <html><head><title>Red Maxi Dress</title></head><body>
      <img src="/logo.png" alt="Site Logo" width="200" height="80">
      <img src="/product.jpg" alt="Red Maxi Dress front view" width="50" height="50">
    </body></html>
    """
    product = parse_product(html, BASE)
    assert product.image_url == "https://shop.example.com/product.jpg"


def test_product_text_dedupes_title_repeated_in_description():
    p = Product("Acme Dress", "Acme Dress — a flowing red maxi dress.", None)
    assert p.text() == "Acme Dress — a flowing red maxi dress."


def test_product_text_joins_distinct_title_and_description():
    p = Product("Acme Dress", "A flowing red maxi dress.", None)
    assert p.text() == "Acme Dress — A flowing red maxi dress."
