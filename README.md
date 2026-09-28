<div align="center">

<img src="data/io.github.peterwalker78.NightSky.svg" width="112" alt="A crescent Moon and a few stars, drawn in dots">

# Night Sky

### Tonight's real sky, and then the real sky outside.

A small stargazing game that is designed to let you go.<br>
Nothing you write ever leaves your computer.

<img src="data/screenshots/sky.png" alt="The night sky drawn in fine dots, with the Milky Way running through it and a soft ring in the middle around Saturn">

</div>

## What it is

Night Sky is a few quiet minutes at the end of the day, to wind down and come
back to what matters to you: the people who are there for you, something to
look forward to, and setting down what's weighing on you so it's lighter in the
morning. The sky is the way in, and its stories are about us as much as about
the stars.

It draws the sky over you right now, in fine dots: the real stars, the
Moon at its real phase, the planets that are really up, the Milky Way where it
really runs. Sweep across it and catch the night's finds: planets, clusters, galaxies, named stars and
constellations to find by their shape, listed at the top right with where to
look for each as the sky turns. Hold Space on one and the view closes in; a
card says something true about it, and something new each time you find it
again.

Some finds take a few minutes:

- **Tonight's story.** Myth and science about stars that are really up, told a
  card at a time while the view moves round them. Each ends on a thought
  turned from the sky back to everyday life.
- **A star-hop.** The old way of finding things: from a star you know, one
  step at a time, with the trail drawn as you go.
- **A walk on the Moon.** Close up on the real photograph at tonight's phase,
  the wisp flies to craters, seas and landing sites near the edge of night,
  where the shadows are long. The line moves every night, so no two walks are
  the same.
- **Things that change.** Jupiter's four big moons in their real places, Venus
  and Mercury at their real phase, Algol dimming on its real schedule, and on
  dark nights one fainter thing to zoom in on, from a list of over a hundred.

Each visit slows down as you play, dims for a real reason, and ends by naming
one thing you can go outside and see tonight.

**The wisp keeps you company.** It sits on a tuft of moss on the horizon: the
same wisp that lives in the [Glimmerwood](https://github.com/peterwalker78/glimmerwood)
browser, out for the night. On your first visit it shows you around. After
that it keeps quiet unless you seem stuck or ask, with `?` or a click on it. It
brightens when you catch something, and falls asleep when the lights go out.

## It wants you to leave

Most apps are built to keep you. This one is built to let you go, and it says
so here so there's no trick in it.

- **The sky is finite.** There are only so many things to find each night.
  Once they're found, the sky is done until it has turned. Tomorrow really is
  different: the Moon moves about thirteen degrees a night.
- **It slows as you play.** The twinkle, the drift and the pace ease down over
  a visit, towards the speed of slow breathing. Nothing announces it.
- **It dims before the end.** Eyes take about twenty minutes to open fully to
  the dark, so the sky darkens to help yours begin.
- **The last line points outside.** *"If it's clear: Saturn is up in the
  south-west, the brightest thing there, and it doesn't twinkle. Give your eyes
  twenty minutes."* Then the window closes itself.
- **Nothing to come back for.** No streaks, scores, badges, levels,
  notifications or counts of nights. Missing a night costs nothing.

## The quiet part

As the sky comes up it asks if anything is heavy tonight. Write two or three
things and each becomes a warm star you hang low in the west. At the end the
sky time-lapses through the rest of the night, and you watch them set.

Now and then, at most twice a visit and never on your first night, the sky asks
a question as you catch something. They are small and specific, and they tend
to be about people, and about things to look forward to. When the answer is a
name, that person can have a star.

Now and then the sky brings back a weight from a week or a month ago and asks
how it sits now: lighter, the same, heavier, or behind you.

Everything goes into a **logbook**: a page for each night, and contents pages
that gather what keeps coming back, including the people whose names come up
again and again. Any weight can have a **course** charted for it, only if you
ask: a wish, the best of it, what gets in the way, and an if-then plan, with
your own earlier words to hand.

If anything you write sounds like real trouble, a helpline sits quietly beside
the sky: Samaritans 116 123 in the UK and Ireland, or findahelpline.com
anywhere else. Nothing is sent or flagged. It is a stargazing game with a
journal, not therapy or a crisis service.

<img src="data/screenshots/moon.png" alt="The Moon close up, drawn in dots, with its terminator and darker seas; beside it a card reads: The Moon. First quarter, 403,000 km away tonight">

## Keys

| Key | |
| --- | --- |
| Arrows, or drag | Look around |
| Scroll, `+` and `-` | Zoom |
| Hold `Space` | Catch what's in the ring |
| `Tab`, or click one in the list | Turn towards the next find |
| `C` | Draw a constellation: arrows step between stars, `Enter` joins, `C` finishes |
| `L` | The logbook |
| `?`, or click the wisp | What the keys do |
| `M` | Music off or on |
| `Esc` twice | End tonight's sky |
| `Ctrl` `,` | Settings |
| `F11` | Full screen |

## Private by design

- The Flatpak has **no network permission**, so nothing can leave, by
  construction.
- **Where you are** comes from your time zone's entry in the tz database, with
  no location service. It can be a few hundred kilometres out, which moves the
  sky by a few degrees; Settings takes a latitude and longitude if you want it
  closer.
- The logbook is **plain files**: a Markdown page per night and a few small
  TOML files, in `~/.var/app/io.github.peterwalker78.NightSky/data/night-sky/`.
  Settings can save a copy anywhere, or forget everything.

## Get it

Download `night-sky-x86_64.flatpak` from the
[latest release](https://github.com/peterwalker78/night-sky/releases/latest)
and install it:

```sh
flatpak install --user night-sky-x86_64.flatpak
```

It uses the GNOME 50 runtime from Flathub, which Flatpak fetches if it isn't
there. It ships with the [Slipstream](https://github.com/peterwalker78/slipstream-desktop)
desktop and runs on any Linux desktop.

## Build it

Rust (stable) and GTK 4.20 or newer.

```sh
scripts/check          # formatting, lints and tests
scripts/dev            # run from the source tree
scripts/flatpak        # build and install the Flatpak for this user
```

Any night can be tried out: `scripts/dev --at=2026-12-14T21:00 --speed=10`
starts the sky at the Geminids and runs it ten times faster, `--quick=10`
shortens the visit itself, `--place=LAT,LON` stands somewhere else, and
`--data=DIR` keeps everything in a scratch folder.

## Where the sky comes from

- **Stars:** the Yale Bright Star Catalogue, 5th revised edition (Hoffleit and
  Warren), from the [CDS](https://cdsarc.cds.unistra.fr/viz-bin/cat/V/50):
  every star to magnitude 6.5.
- **Constellation figures:** [d3-celestial](https://github.com/ofrohn/d3-celestial)
  by Olaf Frohn (BSD-3-Clause), matched to catalogue stars.
- **The Sun, Moon and planets:** mean orbital elements and their largest
  perturbations, after Paul Schlyter's
  [method](https://stjarnhimlen.se/comp/ppcomp.html). The tests check them
  against JPL Horizons: planets within 0.2°, the Moon within 0.1° as seen from
  the ground.
- **Meteor showers:** the International Meteor Organization's working list of
  visual meteor showers.
- **Deep-sky objects:** positions from [SIMBAD](https://simbad.cds.unistra.fr/);
  magnitudes, sizes and facts checked against each object's Wikipedia article.
- **Jupiter's moons:** the lower-accuracy method in Jean Meeus's *Astronomical
  Algorithms*, checked against JPL Horizons.
- **The Moon's features:** the IAU Gazetteer of Planetary Nomenclature, and
  NASA's NSSDCA for the landing sites.
- **Algol's eclipses:** Kreiner's ephemeris.

## The photographs

Once something has been found, zooming in on it lets a real photograph take
over from the dots, at its true size on the sky and turned as it sits there
tonight, with the view following it as the sky turns; the Moon and Mercury
wear tonight's real phase. The 44 pictures come from NASA (public
domain), ESA/Hubble, ESO and NOIRLab/NSF/AURA (CC BY 4.0), and a few from
Wikimedia Commons (CC0, public domain, CC BY 2.0/4.0 and CC BY-SA 4.0).
Each one's credit shows while it's in view, and every title, credit, licence and source
page is in [`app/data/images/credits.toml`](app/data/images/credits.toml) and
in Settings. The pictures are cropped and resized; the CC BY-SA ones stay
under CC BY-SA.

## The music

Six slow lo-fi tracks, all dedicated to the public domain under CC0: "Ease
into Night", "Moon Unit", "Into The Mist" and "Calm Currents" by HoliznaCC0,
and "Chill lofi inspired" and "Lofi Hip Hop Loop" by omfgdude. Sources are in
[`app/data/music/CREDITS.md`](app/data/music/CREDITS.md). The music arrives
with the sky, eases down as it dims and goes with the lights; `M` turns it off
or on.

## Licence

GPL-3.0-or-later. The packed star and constellation data carry their own
credits above.

AI coding tools are used in writing Night Sky's code.
