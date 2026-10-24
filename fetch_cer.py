# Copyright (c) 2026 Archie C. Chapman and contributors
# SPDX-License-Identifier: BSD-3-Clause
"""Download the Clean Energy Regulator's small-scale installation data.

Fetches the postcode files for batteries and solar PV (installations and
capacity, by month) and the historical voluntary battery counts by state,
into raw/cer/<YYYY-MM-DD>/, with each file's URL, size and SHA-256 in
sources.json there.

    python fetch_cer.py [--out raw/cer]

CER content is licensed under Creative Commons Attribution 4.0
(https://cer.gov.au/about-us/our-policies/copyright).
"""

import argparse
import datetime as dt
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

BASE = "https://cer.gov.au/document/"
FILES = {
    "sgu-battery-installations-2011-to-present-and-totals": "csv",
    "sgu-battery-capacity-2011-to-present-and-totals": "csv",
    "sgu-solar-installations-2011-to-present-and-totals": "csv",
    "sgu-solar-capacity-2011-to-present-and-totals": "csv",
    "historical-voluntary-state-data-battery-installation-small-scale-systems": "csv",
}
USER_AGENT = "nem-lens-der/0.1 (+https://github.com/nem-lens/der)"


def fetch(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=Path(__file__).parent / "raw" / "cer")
    args = ap.parse_args()
    dest = args.out / dt.date.today().isoformat()
    dest.mkdir(parents=True, exist_ok=True)
    sources = {}
    for name, ext in FILES.items():
        url = BASE + name
        body = fetch(url)
        path = dest / f"{name}.{ext}"
        path.write_bytes(body)
        sources[path.name] = {
            "url": url,
            "bytes": len(body),
            "sha256": hashlib.sha256(body).hexdigest(),
            "fetched": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        }
        print(f"{path.name}: {len(body):,} bytes")
    (dest / "sources.json").write_text(json.dumps(sources, indent=2))
    print(f"wrote {dest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
