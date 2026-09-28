#!/usr/bin/env python3
"""Packs the star catalogue and constellation figures into the compact files
the app embeds. Run once when the sources change; the outputs are committed.

Sources (downloaded to tools/raw/ by scripts/pack-data):
  bsc5.dat.gz                Yale Bright Star Catalogue, 5th revised edition
                             (Hoffleit & Warren 1991), CDS catalogue V/50
  constellations.lines.json  d3-celestial by Olaf Frohn, BSD-3-Clause
  constellations.json        d3-celestial, for English names and ranks

Outputs:
  core/data/stars.bin            14-byte little-endian records, brightest first:
                                 u16 HR, f32 RA (deg, J2000), f32 Dec (deg),
                                 i16 V magnitude x100, i16 B-V x100
                                 (B-V of 99.99 means unknown)
  core/data/constellations.txt   one figure per line:
                                 abbrev|English name|rank|a-b a-b ...
                                 where a and b are HR numbers
"""

import gzip
import json
import math
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RAW = ROOT / "tools" / "raw"
OUT = ROOT / "core" / "data"


def field(line, start, end):
    """Bytes start..end, 1-based and inclusive, as in the catalogue's ReadMe."""
    return line[start - 1 : end].strip()


def stars():
    rows = []
    with gzip.open(RAW / "bsc5.dat.gz", "rt", encoding="latin-1") as f:
        for line in f:
            line = line.rstrip("\n").ljust(197)
            ra_h = field(line, 76, 77)
            vmag = field(line, 103, 107)
            if not ra_h or not vmag:
                continue  # novae and non-stellar objects kept for numbering
            ra = 15 * (int(ra_h) + int(field(line, 78, 79)) / 60 + float(field(line, 80, 83)) / 3600)
            dec = int(field(line, 85, 86)) + int(field(line, 87, 88)) / 60 + int(field(line, 89, 90)) / 3600
            if field(line, 84, 84) == "-":
                dec = -dec
            bv = field(line, 110, 114)
            rows.append(
                (
                    int(field(line, 1, 4)),
                    ra,
                    dec,
                    round(float(vmag) * 100),
                    round(float(bv) * 100) if bv else 9999,
                )
            )
    rows.sort(key=lambda r: r[3])
    return rows


def angle(a, b):
    ra1, de1, ra2, de2 = map(math.radians, (a[0], a[1], b[0], b[1]))
    c = math.sin(de1) * math.sin(de2) + math.cos(de1) * math.cos(de2) * math.cos(ra1 - ra2)
    return math.degrees(math.acos(max(-1.0, min(1.0, c))))


def nearest(point, catalogue):
    best = min(catalogue, key=lambda s: angle(point, (s[1], s[2])))
    return best, angle(point, (best[1], best[2]))


def figures(catalogue):
    names = {f["id"]: f["properties"] for f in json.load(open(RAW / "constellations.json"))["features"]}
    lines = json.load(open(RAW / "constellations.lines.json"))["features"]
    # Figures only use stars a person can see, so matching against the
    # brighter part of the catalogue is both quicker and safer.
    bright = [s for s in catalogue if s[3] <= 650]
    out, misses = [], 0
    for feature in lines:
        edges = []
        for strand in feature["geometry"]["coordinates"]:
            hrs = []
            for lon, lat in strand:
                star, off = nearest((lon % 360, lat), bright)
                if off > 0.25:
                    misses += 1
                    hrs.append(None)
                else:
                    hrs.append(star[0])
            for a, b in zip(hrs, hrs[1:]):
                if a and b and a != b and (a, b) not in edges and (b, a) not in edges:
                    edges.append((a, b))
        props = names[feature["id"]]
        pairs = " ".join(f"{a}-{b}" for a, b in edges)
        out.append(f"{feature['id']}|{props['en']}|{props['rank']}|{pairs}")
    return out, misses


def main():
    catalogue = stars()
    with open(OUT / "stars.bin", "wb") as f:
        for hr, ra, dec, vmag, bv in catalogue:
            f.write(struct.pack("<Hffhh", hr, ra, dec, vmag, bv))
    figs, misses = figures(catalogue)
    (OUT / "constellations.txt").write_text(
        "# Constellation figures from d3-celestial (c) 2015 Olaf Frohn, BSD-3-Clause,\n"
        "# with each vertex matched to its Yale Bright Star Catalogue HR number.\n"
        "# abbrev|English name|rank|HR-HR pairs\n" + "\n".join(figs) + "\n"
    )
    print(f"{len(catalogue)} stars, {len(figs)} figures, {misses} vertices unmatched", file=sys.stderr)


main()
