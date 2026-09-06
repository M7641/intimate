#!/usr/bin/env python3
"""Scrape product image + description from a retail product page.

Standalone helper that feeds the `extract` pipeline. It is *site-agnostic*: it
reads structured metadata that most retailers embed, in order of reliability:

    1. JSON-LD  <script type="application/ld+json">  with @type "Product"
    2. Open Graph meta tags  (og:title / og:description / og:image)
    3. Plain <meta name="description"> and <title>

Output is a CSV with the columns the pipeline expects (id, description,
image_path), plus title / image_url / source_url for traceability. Images are
downloaded into --image-dir.

    uv run extract-scrape URL [URL ...] --out products.csv --image-dir images

Etiquette: robots.txt is honoured by default and there is a delay between
requests. Keep volumes low. Amazon and other large retailers frequently block
non-browser traffic and may serve a CAPTCHA page instead of the product — for a
reliable demo, sandbox sites such as https://books.toscrape.com are ideal.

uv run extract-scrape "https://books.toscrape.com/catalogue/a-light-in-the-attic_1000/index.html"
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
import time
from pathlib import Path
from urllib.parse import urljoin, urlparse
from urllib.robotparser import RobotFileParser

import requests
from bs4 import BeautifulSoup

UA = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/124.0 Safari/537.36"
)

_EXT_BY_TYPE = {
    "image/jpeg": ".jpg",
    "image/png": ".png",
    "image/webp": ".webp",
    "image/gif": ".gif",
}


class Product:
    def __init__(self, title: str, description: str, image_url: str | None):
        self.title = title.strip()
        self.description = description.strip()
        self.image_url = image_url

    def text(self) -> str:
        """Combined text handed to the VLM (title gives the model context)."""
        parts = [p for p in (self.title, self.description) if p]
        # Drop a description that merely repeats the title.
        if len(parts) == 2 and parts[1].startswith(parts[0]):
            parts = [parts[1]]
        return " — ".join(dict.fromkeys(parts))


def robots_allows(url: str, user_agent: str) -> bool:
    parts = urlparse(url)
    robots_url = f"{parts.scheme}://{parts.netloc}/robots.txt"
    rp = RobotFileParser()
    rp.set_url(robots_url)
    try:
        rp.read()
    except Exception:
        # No reachable robots.txt → treat as allowed (common for sandboxes).
        return True
    return rp.can_fetch(user_agent, url)


def fetch(url: str, *, timeout: float = 20.0) -> str:
    resp = requests.get(url, headers={"User-Agent": UA}, timeout=timeout)
    resp.raise_for_status()
    return resp.text


def _iter_jsonld(soup: BeautifulSoup):
    for tag in soup.find_all("script", type="application/ld+json"):
        raw = tag.string or tag.get_text()
        if not raw:
            continue
        try:
            data = json.loads(raw)
        except json.JSONDecodeError:
            continue
        # JSON-LD may be a single object, a list, or wrapped in @graph.
        if isinstance(data, dict) and "@graph" in data:
            yield from data["@graph"]
        elif isinstance(data, list):
            yield from data
        else:
            yield data


def _is_product(node: object) -> bool:
    if not isinstance(node, dict):
        return False
    t = node.get("@type", "")
    types = t if isinstance(t, list) else [t]
    return any("Product" in str(x) for x in types)


def _jsonld_image(value: object) -> str | None:
    if isinstance(value, str):
        return value
    if isinstance(value, list) and value:
        return _jsonld_image(value[0])
    if isinstance(value, dict):
        return value.get("url") or value.get("contentUrl")
    return None


def parse_product(html: str, base_url: str) -> Product:
    soup = BeautifulSoup(html, "lxml")
    title = description = ""
    image: str | None = None

    # 1. JSON-LD Product — most reliable when present.
    for node in _iter_jsonld(soup):
        if _is_product(node):
            title = title or str(node.get("name", ""))
            description = description or str(node.get("description", ""))
            image = image or _jsonld_image(node.get("image"))
            break

    # 2. Open Graph fallbacks.
    def meta(prop: str, attr: str = "property") -> str | None:
        tag = soup.find("meta", attrs={attr: prop})
        return tag.get("content") if tag and tag.get("content") else None

    title = title or meta("og:title") or (soup.title.string if soup.title else "") or ""
    description = (
        description or meta("og:description") or meta("description", attr="name") or ""
    )
    image = image or meta("og:image") or meta("twitter:image", attr="name")

    # 3. Last resort: the <img> whose alt text best matches the product title
    #    (falling back to the largest one). The product image describes the
    #    product; borders, logos and chrome do not.
    image = image or _fallback_image(soup, title)

    if image:
        image = urljoin(base_url, image)
    return Product(title, description, image)


_STOPWORDS = frozenset(
    "the a an and or for with of to in on by new gaming series edition".split()
)


def _tokens(text: str) -> set[str]:
    return {
        w for w in re.findall(r"[a-z0-9]+", text.lower())
        if len(w) >= 3 and w not in _STOPWORDS
    }


def _best_src(img) -> str | None:
    """Highest-resolution URL on an <img>, handling lazy-loading attributes."""
    if img.get("data-old-hires"):
        return img["data-old-hires"]
    srcset = img.get("srcset") or img.get("data-srcset")
    if srcset:
        # "url1 300w, url2 600w" / "url1 1x, url2 2x" → take the last (largest).
        return srcset.split(",")[-1].strip().split(" ")[0]
    return img.get("src") or img.get("data-src")


def _fallback_image(soup: BeautifulSoup, title: str = "") -> str | None:
    skip = ("logo", "icon", "sprite", "pixel", "spacer", "avatar", "swatch")
    title_tokens = _tokens(title)
    # (alt-match score, area) — higher is better; alt match dominates.
    best: tuple[int, int, str] | None = None
    for img in soup.find_all("img"):
        src = _best_src(img)
        if not src or src.startswith("data:") or src.lower().endswith(".svg"):
            continue
        if any(word in src.lower() for word in skip):
            continue
        alt_score = len(title_tokens & _tokens(img.get("alt", "")))
        try:
            area = int(img.get("width", 0)) * int(img.get("height", 0))
        except (TypeError, ValueError):
            area = 0
        candidate = (alt_score, area, src)
        if best is None or candidate[:2] > best[:2]:
            best = candidate
    return best[2] if best else None


def download_image(url: str, dest_stem: Path, *, timeout: float = 20.0) -> Path:
    resp = requests.get(url, headers={"User-Agent": UA}, timeout=timeout)
    resp.raise_for_status()
    ext = _EXT_BY_TYPE.get(resp.headers.get("content-type", "").split(";")[0].strip())
    if ext is None:
        ext = Path(urlparse(url).path).suffix or ".jpg"
    dest = dest_stem.with_suffix(ext)
    dest.write_bytes(resp.content)
    return dest


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("urls", nargs="+", help="product page URL(s)")
    ap.add_argument("--out", type=Path, default=Path("products.csv"), help="output CSV")
    ap.add_argument(
        "--image-dir", type=Path, default=Path("images"), help="image download dir"
    )
    ap.add_argument("--delay", type=float, default=2.0, help="seconds between requests")
    ap.add_argument(
        "--ignore-robots", action="store_true", help="skip robots.txt check"
    )
    args = ap.parse_args(argv)

    args.image_dir.mkdir(parents=True, exist_ok=True)
    rows: list[dict] = []

    for i, url in enumerate(args.urls, start=1):
        if i > 1:
            time.sleep(args.delay)
        if not args.ignore_robots and not robots_allows(url, UA):
            print(f"[skip] robots.txt disallows {url}", file=sys.stderr)
            continue
        try:
            product = parse_product(fetch(url), url)
        except requests.RequestException as exc:
            print(f"[error] fetch failed for {url}: {exc}", file=sys.stderr)
            continue

        if not product.text():
            print(f"[warn] no product metadata found at {url}", file=sys.stderr)

        image_path = ""
        if product.image_url:
            try:
                image_path = str(
                    download_image(product.image_url, args.image_dir / f"{i}")
                )
            except requests.RequestException as exc:
                print(f"[warn] image download failed for {url}: {exc}", file=sys.stderr)

        rows.append(
            {
                "id": i,
                "description": product.text(),
                "image_path": image_path,
                "title": product.title,
                "image_url": product.image_url or "",
                "source_url": url,
            }
        )
        print(f"[ok] {url} → {product.title[:60]!r}", file=sys.stderr)

    if not rows:
        print("No products scraped.", file=sys.stderr)
        return 1

    with args.out.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(
            fh,
            fieldnames=[
                "id",
                "description",
                "image_path",
                "title",
                "image_url",
                "source_url",
            ],
        )
        writer.writeheader()
        writer.writerows(rows)
    print(f"Wrote {len(rows)} rows → {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
