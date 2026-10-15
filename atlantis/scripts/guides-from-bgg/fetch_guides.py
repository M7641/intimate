"""Fetch the hero guides linked from the hero sheet into docs/guides/.

The sheet's Hero column links each hero to a BoardGameGeek forum thread.
For each link: download the thread, keep the guide (the first post, plus
what its author posted straight after it), and write it out as Markdown.

Run from the atlantis directory:

    python3 scripts/guides-from-bgg/fetch_guides.py

Downloads are cached in --cache (default: atlantis-guides-cache in the
system temp directory); delete it to fetch again.
"""

import argparse
import json
import re
import tempfile
import textwrap
import time
import urllib.request
import zipfile
from pathlib import Path

SHEET_ID = "1YtYpP6IA1OngUKPJ8hxpkIqdhVNI8fq9EA2tIdf0TkI"
SHEET_URL = f"https://docs.google.com/spreadsheets/d/{SHEET_ID}"
# The JSON the forum pages load themselves. BGG's XML API needs a token.
API = "https://api.geekdo.com/api"
USER_AGENT = "Mozilla/5.0 (X11; Linux x86_64) atlantis-guides/1.0"
# Seconds between requests. Thirty threads: no need to hurry.
PAUSE = 1.5
# An author's post straight after the guide counts as part of it when it is
# longer than a passing remark.
FOLLOW_UP_MIN_CHARS = 400
WIDTH = 80


def download(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read()


def cached_json(cache: Path, name: str, url: str):
    path = cache / name
    if not path.exists():
        path.write_bytes(download(url))
        time.sleep(PAUSE)
    return json.loads(path.read_text())


def hero_links(cache: Path) -> list[tuple[str, str | None]]:
    """(hero name, guide URL or None) for every row of the Heroes sheet.

    A CSV export drops hyperlinks, so read them from the xlsx: the links sit
    in the sheet's relationships, keyed by cell.
    """
    path = cache / "sheet.xlsx"
    if not path.exists():
        path.write_bytes(download(f"{SHEET_URL}/export?format=xlsx"))
    book = zipfile.ZipFile(path)
    sheet = book.read("xl/worksheets/sheet1.xml").decode()
    rels = book.read("xl/worksheets/_rels/sheet1.xml.rels").decode()
    strings = [
        "".join(re.findall(r"<t[^>]*>([^<]*)</t>", item))
        for item in re.findall(r"<si>(.*?)</si>", book.read("xl/sharedStrings.xml").decode(), re.S)
    ]

    def attrs(tag: str) -> dict[str, str]:
        return dict(re.findall(r'([\w:]+)="([^"]*)"', tag))

    targets = {a["Id"]: a["Target"] for a in map(attrs, re.findall(r"<Relationship [^>]*>", rels))}
    links = {a["ref"]: targets[a["r:id"]] for a in map(attrs, re.findall(r"<hyperlink [^>]*>", sheet))}

    heroes = []
    for cell, index in re.findall(r'<c r="(A\d+)"[^>]*t="s"[^>]*><v>(\d+)</v>', sheet):
        if cell != "A1":
            heroes.append((strings[int(index)], links.get(cell)))
    return heroes


def hero_key(name: str) -> str:
    """Same rule as core/src/hero.rs: "Wasp the Warmaiden" -> "wasp"."""
    return name.split()[0].lower()


# --- BBCode to Markdown -----------------------------------------------------

TAG = re.compile(r"\[(/?)(b|i|u|heading|size|center|color|bgcolor)(?:=([^\]]*))?\]", re.I)
IMAGE = re.compile(r"\[img\](.*?)\[/img\]", re.I)
BGG_IMAGE = re.compile(r"\[imageid=(\d+)[^\]]*\]", re.I)
LINK = re.compile(r"\[url=([^\]]+)\](.*?)\[/url\]", re.I | re.S)
BULLET = re.compile(r"^(?:[-•]\s+)")


def emphasis(text: str, bold: bool, italic: bool) -> str:
    """Wrap `text` in markers, keeping its outer whitespace outside them."""
    core = text.strip()
    if not core or not (bold or italic):
        return text
    mark = "**" * bold + "*" * italic
    lead = text[: len(text) - len(text.lstrip())]
    tail = text[len(text.rstrip()) :]
    return f"{lead}{mark}{core}{mark}{tail}"


def styled(runs: list[tuple]) -> str:
    """One line's runs as Markdown.

    A style shared by the whole line wraps it once; only what differs is
    marked inside. That keeps markers from piling up where runs touch.
    """
    merged: list[list] = []
    for text, bold, italic, *_ in runs:
        text = text.replace("*", r"\*")
        if merged and (not text.strip() or merged[-1][1:] == [bold, italic]):
            merged[-1][0] += text
        else:
            merged.append([text, bold, italic])
    inked = [m for m in merged if m[0].strip()]
    all_bold = all(m[1] for m in inked)
    all_italic = all(m[2] for m in inked)
    inner = "".join(emphasis(t, b and not all_bold, i and not all_italic) for t, b, i in merged)
    return emphasis(inner.strip(), all_bold, all_italic)


def heading_level(size: int | None) -> int:
    """Guides sit under one `#` title, so their own headings start at `##`."""
    if size is None:
        return 3
    return 2 if size >= 22 else 3 if size >= 18 else 4


def wrap(paragraph: str) -> str:
    bullet = BULLET.match(paragraph)
    if bullet:
        paragraph = "- " + paragraph[bullet.end() :]
    return textwrap.fill(
        paragraph,
        WIDTH,
        subsequent_indent="  " if bullet else "",
        break_long_words=False,
        break_on_hyphens=False,
    )


def to_markdown(bbcode: str) -> str:
    """One BBCode line is one paragraph. Tags may stay open across lines."""
    text = bbcode.replace("\r", "")
    text = LINK.sub(lambda m: f"[{m.group(2).strip()}]({m.group(1)})", text)
    # Embedded images are dropped: their host, imgur, blocks whole regions.
    text = IMAGE.sub("", text)
    text = BGG_IMAGE.sub(
        lambda m: f"\n[Image {m.group(1)} on BoardGameGeek](https://boardgamegeek.com/image/{m.group(1)})\n",
        text,
    )

    depth = {"b": 0, "i": 0, "heading": 0}
    sizes: list[int] = []
    blocks = []
    for line in text.split("\n"):
        # (text, bold, italic, set as a heading, font size) per run.
        runs = []
        position = 0
        for tag in TAG.finditer(line):
            if tag.start() > position:
                chunk = line[position : tag.start()]
                runs.append((chunk, depth["b"] > 0, depth["i"] > 0, depth["heading"] > 0, sizes[-1] if sizes else None))
            position = tag.end()
            closing, name, value = tag.group(1), tag.group(2).lower(), tag.group(3)
            if name in depth:
                depth[name] = max(0, depth[name] + (-1 if closing else 1))
            elif name == "size":
                if closing and sizes:
                    sizes.pop()
                elif not closing:
                    sizes.append(int(float(value or 10)))
        if position < len(line):
            runs.append((line[position:], depth["b"] > 0, depth["i"] > 0, depth["heading"] > 0, sizes[-1] if sizes else None))

        inked = [r for r in runs if r[0].strip()]
        if not inked:
            continue
        plain = "".join(r[0] for r in runs).strip()
        if plain.startswith("[Image "):
            blocks.append(plain)
        elif all(r[3] or (r[4] or 0) >= 16 for r in inked) and len(plain) <= 100:
            size = max((r[4] for r in inked if r[4]), default=None)
            blocks.append(f"{'#' * heading_level(size)} {plain}")
        else:
            blocks.append(wrap(re.sub(r"^#", r"\\#", styled(runs))))
    return "\n\n".join(blocks)


# --- One guide ---------------------------------------------------------------


def printing(subject: str) -> str:
    lowered = subject.lower()
    if "2nd printing" in lowered or "second printing" in lowered:
        return "second"
    if "1st printing" in lowered or "first printing" in lowered:
        return "first"
    return "not stated"


def guide(cache: Path, hero: str, url: str) -> dict:
    thread_id = re.search(r"/thread/(\d+)", url).group(1)
    thread = cached_json(cache, f"thread-{thread_id}.json", f"{API}/threads/{thread_id}")
    posts = cached_json(
        cache, f"articles-{thread_id}.json", f"{API}/articles?threadid={thread_id}&showcount=50&pageid=1"
    )["articles"]
    author_id = str(thread["creator"])
    author = cached_json(cache, f"user-{author_id}.json", f"{API}/users/{author_id}")["username"]

    first, rest = posts[0], posts[1:]
    follow_ups = []
    for post in rest:
        if str(post["author"]) != author_id:
            break
        if len(post["body"]) >= FOLLOW_UP_MIN_CHARS:
            follow_ups.append(post)

    posted, edited = first["postdate"][:10], first["editdate"][:10]
    lines = [
        f"# {thread['subject']}",
        "",
        f"- Hero: {hero}",
        f"- Author: {author}, on BoardGameGeek",
        f"- Source: <{thread['canonical_link']}>",
        f"- Posted: {posted}" + (f", edited {edited}" if edited != posted else ""),
        f"- Printing: {printing(thread['subject'])}",
        "",
        to_markdown(first["body"]),
    ]
    for post in follow_ups:
        lines += ["", f"## Follow-up by the author ({post['postdate'][:10]})", "", to_markdown(post["body"])]
    return {
        "hero": hero,
        "file": f"{hero_key(hero)}.md",
        "author": author,
        "posted": posted,
        "edited": edited,
        "printing": printing(thread["subject"]),
        "url": thread["canonical_link"],
        "markdown": "\n".join(lines) + "\n",
    }


README = """\
# Hero guides

Community hero guides ("hero spotlights") from the BoardGameGeek strategy
forum, one file per hero. The hero sheet links to them from its Hero column:
<{sheet}>.

Written by `scripts/guides-from-bgg/fetch_guides.py`. Do not edit by hand;
run the script again to refresh.

## Files

| hero | guide | printing | author | posted |
| ---- | ----- | -------- | ------ | ------ |
{rows}

No guide is linked for: {missing}.

## Reading them

- Each guide belongs to its author, named at the top of the file with a link
  to the thread.
- "Printing" is what the thread title says. Where it says nothing, go by the
  date: the guides posted in 2022 predate the second printing.
- The second printing changed many cards, and `data/cards.json` holds the
  second printing: a first-printing guide names cards and values the
  catalogue no longer has.
- A file holds the guide itself: the thread's first post, plus what the
  author posted straight after it. Replies and later discussion are left
  out; follow the source link for those.
- Images are left out. The ones embedded from imgur are dropped, because
  imgur blocks whole regions; the ones hosted on BoardGameGeek remain as a
  link to the image's page.
- Headings come from the post's own formatting, so their levels vary from
  one author to the next.
"""


def readme(index: list[dict]) -> str:
    rows = [
        f"| {e['hero']} | [{e['file']}]({e['file']}) | {e['printing']} | {e['author']} | {e['posted']} |"
        for e in index
        if e["file"]
    ]
    missing = ", ".join(e["hero"] for e in index if e["file"] is None)
    return README.format(sheet=SHEET_URL, rows="\n".join(rows), missing=missing)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, default=Path("docs/guides"))
    parser.add_argument("--cache", type=Path, default=Path(tempfile.gettempdir()) / "atlantis-guides-cache")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    args.cache.mkdir(parents=True, exist_ok=True)

    index = []
    for hero, url in hero_links(args.cache):
        if url is None:
            index.append({"hero": hero, "file": None})
            continue
        entry = guide(args.cache, hero, url)
        (args.out / entry["file"]).write_text(entry.pop("markdown"))
        index.append(entry)
        print(f"{entry['file']:18} {entry['printing']:10} {entry['author']}")
    (args.out / "README.md").write_text(readme(index))
    missing = [e["hero"] for e in index if e["file"] is None]
    print(f"{len(index) - len(missing)} guides written, none linked for: {', '.join(missing)}")

if __name__ == "__main__":
    main()
