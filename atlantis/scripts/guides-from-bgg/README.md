# guides-from-bgg

How `docs/guides/` was made. The hero sheet (the Google Sheet behind
`data/heroes.csv`) links each hero's name to a guide thread on the
BoardGameGeek strategy forum.

```bash
python3 scripts/guides-from-bgg/fetch_guides.py    # from atlantis/
```

Standard library only. The script:

1. downloads the sheet as xlsx and reads the links off the Hero column (a
   CSV export drops them);
2. fetches each thread from `api.geekdo.com`, the JSON the forum pages load
   themselves, one request every 1.5 seconds. BoardGameGeek's documented XML
   API answers 401 without a registered token;
3. converts the guide's BBCode to Markdown and writes `docs/guides/<hero>.md`
   plus the `README.md` index.

Run it again to pick up edited guides, or guides newly linked from the sheet.
