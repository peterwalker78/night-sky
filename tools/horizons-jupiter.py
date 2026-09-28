#!/usr/bin/env python3
"""Reference positions of Jupiter and its four big moons from JPL Horizons,
for the moon-position tests: prints Rust tuples (unix ms, body, ra, dec)."""

import re
import urllib.parse
import urllib.request

TIMES = [1790625600000, 1805079600000, 1909173600000, 2082088800000, 1795000000000, 1800000000000]
BODIES = {"jupiter": "599", "io": "501", "europa": "502", "ganymede": "503", "callisto": "504"}

for ms in TIMES:
    jd = ms / 86400000 + 2440587.5
    for name, code in BODIES.items():
        params = {"format": "text", "COMMAND": f"'{code}'", "EPHEM_TYPE": "'OBSERVER'",
                  "CENTER": "'500@399'", "TLIST": f"'{jd}'", "QUANTITIES": "'2'",
                  "ANG_FORMAT": "'DEG'", "TIME_TYPE": "'UT'", "EXTRA_PREC": "'YES'"}
        url = "https://ssd.jpl.nasa.gov/api/horizons.api?" + urllib.parse.urlencode(params)
        text = urllib.request.urlopen(url, timeout=60).read().decode()
        body = text.split("$$SOE")[1].split("$$EOE")[0]
        nums = re.findall(r"-?\d+\.\d+", body)
        print(f'    ({ms}, "{name}", {float(nums[-2]):.7f}, {float(nums[-1]):.7f}),')
