"""Download AEMO's rooftop PV estimates from the MMSDM monthly archive.

ROOFTOP_PV_ACTUAL is AEMO's estimate of rooftop PV output per NEM region
and half hour. This fetches one zip per month into
raw/aemo/ROOFTOP_PV_ACTUAL/<YYYY_MM>.zip, skipping months already there,
and records each file's URL, size and SHA-256 in sources.json.

    python fetch_aemo.py [--from 2018-01] [--to YYYY-MM] [--out raw/aemo]

The default end is the latest month AEMO has archived (usually two months
back); months not yet published are reported and skipped. AEMO's data is
used under AEMO's copyright permissions
(https://www.aemo.com.au/privacy-and-legal-notices/copyright-permissions).
"""

import argparse
import datetime as dt
import hashlib
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path

ARCHIVE = "https://nemweb.com.au/Data_Archive/Wholesale_Electricity/MMSDM"
TABLE = "ROOFTOP_PV_ACTUAL"
USER_AGENT = "nem-lens-der/0.1 (+https://github.com/nem-lens/der)"


def urls_for(year: int, month: int) -> list[str]:
    """The archive's file names: PUBLIC_ARCHIVE#...#FILE01# from August
    2024, PUBLIC_DVD_... before."""
    stamp = f"{year}{month:02d}010000"
    base = f"{ARCHIVE}/{year}/MMSDM_{year}_{month:02d}/MMSDM_Historical_Data_SQLLoader/DATA/"
    return [
        base + f"PUBLIC_ARCHIVE%23{TABLE}%23FILE01%23{stamp}.zip",
        base + f"PUBLIC_DVD_{TABLE}_{stamp}.zip",
    ]


def months(start: str, end: str):
    y, m = map(int, start.split("-"))
    ey, em = map(int, end.split("-"))
    while (y, m) <= (ey, em):
        yield y, m
        y, m = (y + 1, 1) if m == 12 else (y, m + 1)


def main() -> int:
    today = dt.date.today()
    ap = argparse.ArgumentParser()
    ap.add_argument("--from", dest="start", default="2018-01")
    ap.add_argument("--to", dest="end", default=f"{today.year}-{today.month:02d}")
    ap.add_argument("--out", type=Path, default=Path(__file__).parent / "raw" / "aemo")
    args = ap.parse_args()
    dest = args.out / TABLE
    dest.mkdir(parents=True, exist_ok=True)
    manifest = dest / "sources.json"
    sources = json.loads(manifest.read_text()) if manifest.exists() else {}
    for y, m in months(args.start, args.end):
        path = dest / f"{y}_{m:02d}.zip"
        if path.exists():
            continue
        body = None
        for url in urls_for(y, m):
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            try:
                with urllib.request.urlopen(req, timeout=120) as r:
                    body = r.read()
                break
            except urllib.error.HTTPError as e:
                if e.code != 404:
                    raise
        if body is None:
            print(f"{y}-{m:02d}: not published")
            continue
        path.write_bytes(body)
        sources[path.name] = {
            "url": url,
            "bytes": len(body),
            "sha256": hashlib.sha256(body).hexdigest(),
            "fetched": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        }
        print(f"{y}-{m:02d}: {len(body):,} bytes", flush=True)
        manifest.write_text(json.dumps(sources, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
