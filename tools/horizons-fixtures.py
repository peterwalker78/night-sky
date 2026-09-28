#!/usr/bin/env python3
"""Fetches reference positions from JPL Horizons for the ephemeris tests and
prints them as Rust tuples: (unix ms, body, ra, dec). Planets and the Sun are
geocentric; the Moon is topocentric for the site given with it."""

import re
import sys
import urllib.parse
import urllib.request

DATES = [
    ("2026-09-28 20:00", 1790625600000),
    ("2027-03-15 03:00", 1805079600000),
    ("2030-07-01 22:00", 1909173600000),
    ("2035-12-24 06:00", 2082088800000),
    ("2045-05-05 12:00", 2377598400000),
]
BODIES = {"Sun": "10", "Mercury": "199", "Venus": "299", "Mars": "499", "Jupiter": "599",
          "Saturn": "699", "Uranus": "799", "Neptune": "899", "Moon": "301"}
SITES = [("london", -0.1276, 51.5072), ("sydney", 151.2093, -33.8688), ("tromso", 18.9553, 69.6492)]


def query(command, center, jd, site=None):
    params = {
        "format": "text", "COMMAND": f"'{command}'", "EPHEM_TYPE": "'OBSERVER'",
        "CENTER": f"'{center}'", "TLIST": f"'{jd}'", "QUANTITIES": "'2'",
        "ANG_FORMAT": "'DEG'", "TIME_TYPE": "'UT'", "EXTRA_PREC": "'YES'",
    }
    if site:
        params["COORD_TYPE"] = "'GEODETIC'"
        params["SITE_COORD"] = f"'{site[1]},{site[2]},0'"
    url = "https://ssd.jpl.nasa.gov/api/horizons.api?" + urllib.parse.urlencode(params)
    text = urllib.request.urlopen(url, timeout=60).read().decode()
    body = text.split("$$SOE")[1].split("$$EOE")[0].strip()
    nums = re.findall(r"-?\d+\.\d+", body)
    return float(nums[-2]), float(nums[-1])


for label, ms in DATES:
    jd = ms / 86400000 + 2440587.5
    for name, code in BODIES.items():
        if name == "Moon":
            for site in SITES:
                ra, dec = query(code, "coord@399", jd, site)
                print(f'    ({ms}, "moon@{site[0]}", {ra:.5f}, {dec:.5f}), // {label}')
        else:
            ra, dec = query(code, "500@399", jd)
            print(f'    ({ms}, "{name.lower()}", {ra:.5f}, {dec:.5f}), // {label}')
    sys.stdout.flush()
