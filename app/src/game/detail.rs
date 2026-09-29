//! Free look's cards. A click on anything tells all there is: every fact
//! kept about it, where it is now, how bright and how big, and its
//! photograph when there is one. The wisp keeps out of it.

use super::*;

/// What a click in free look is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Subject {
    /// One of tonight's finds.
    Find(usize),
    Body(Body),
    /// A showpiece or a deep-sky object, by index.
    Piece(usize),
    /// A named star, by HR number.
    Star(u16),
    /// Something with nothing more to tell.
    Plain,
}

/// The most paragraphs a card holds, so it stays on the screen.
const MOST_PARAGRAPHS: usize = 5;

impl Game {
    /// Everything to say about a subject: kicker, title, the paragraphs, and
    /// a photograph's id if there is one.
    pub(crate) fn detail(
        &self,
        subject: Subject,
        now: UnixMs,
    ) -> Option<(String, String, String, Option<String>)> {
        let hz = horizon(self.observer, now);
        let prec = precession(now);
        let year = civil_date(now, self.offset_s).0;
        let place = |v: Vec3| {
            let (alt, az) = alt_az(v);
            westering_core::finale::whereabouts(alt, az)
        };
        let photo = |id: &str| self.photos.credit(id).map(|_| id.to_owned());
        let (kicker, title, mut paragraphs, picture) = match subject {
            Subject::Find(i) => {
                let find = &self.finds[i];
                let here = self.find_dir(i, now, &hz, &prec).map(place);
                match find.target {
                    Target::Body(b) => {
                        let mut all = vec![find.fact.clone()];
                        all.extend(self.body_facts(b));
                        (
                            self.kind_word(i).to_owned(),
                            find.name.clone(),
                            all,
                            photo(b.id()),
                        )
                            .with_place(here)
                    }
                    Target::Showpiece(p) => {
                        let (k, t, all, pic) = self.piece_detail(p);
                        (k, t, all, pic).with_place(here)
                    }
                    Target::Star(hr) => {
                        let (k, t, all, pic) = self.star_detail(hr, year)?;
                        (k, t, all, pic).with_place(here)
                    }
                    Target::Figure(f) => {
                        let figure = &self.sky.figures[f];
                        let mut all = vec![find.fact.clone()];
                        if let Some(note) =
                            self.sky.notes.iter().find(|n| n.abbrev == figure.abbrev)
                        {
                            all.push(note.fact.clone());
                            all.extend(note.more.iter().cloned());
                        }
                        let pic = self
                            .photos
                            .of_constellation(&figure.abbrev)
                            .map(|c| c.id.clone());
                        ("Constellation".to_owned(), figure.name.clone(), all, pic).with_place(here)
                    }
                    _ => return None,
                }
            }
            Subject::Body(b) => {
                let s = see(b, self.observer, now);
                let name = {
                    let n = b.name();
                    let mut c = n.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                };
                let kind = if b == Body::Moon {
                    "Our Moon"
                } else {
                    "Planet"
                };
                (kind.to_owned(), name, self.body_facts(b), photo(b.id()))
                    .with_place(Some(westering_core::finale::whereabouts(s.alt, s.az)))
            }
            Subject::Piece(p) => {
                let piece = &self.sky.lists.showpieces[p];
                let v = apply(&hz, apply(&prec, unit(piece.ra, piece.dec)));
                let (k, t, all, pic) = self.piece_detail(p);
                (k, t, all, pic).with_place(Some(place(v)))
            }
            Subject::Star(hr) => {
                let v = self
                    .sky
                    .stars
                    .index_of(hr)
                    .map(|idx| apply(&hz, self.star_dirs[idx]));
                let (k, t, all, pic) = self.star_detail(hr, year)?;
                (k, t, all, pic).with_place(v.map(place))
            }
            Subject::Plain => return None,
        };
        paragraphs.dedup();
        paragraphs.truncate(MOST_PARAGRAPHS);
        Some((kicker, title, paragraphs.join("\n\n"), picture))
    }

    fn body_facts(&self, b: Body) -> Vec<String> {
        self.sky
            .lists
            .bodies
            .iter()
            .find(|f| f.id == b.id())
            .map(|f| f.facts.clone())
            .unwrap_or_default()
    }

    fn piece_detail(&self, p: usize) -> (String, String, Vec<String>, Option<String>) {
        let piece = &self.sky.lists.showpieces[p];
        let mut all = vec![piece.fact.clone()];
        all.extend(piece.more.iter().cloned());
        // How bright and how big, when there's a brightness to give.
        let mut numbers = Vec::new();
        if piece.mag < 30.0 {
            numbers.push(format!("magnitude {:.1}", piece.mag));
        }
        if piece.size >= 1.0 {
            numbers.push(format!("about {:.0} arcminutes across", piece.size));
        }
        if !numbers.is_empty() {
            let line = numbers.join(", ");
            let mut c = line.chars();
            all.push(
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
                    + ".",
            );
        }
        let kind = match piece.kind {
            Kind::Cluster => "Star cluster",
            Kind::Galaxy => "Galaxy",
            Kind::Nebula => "Nebula",
            Kind::Double => "Double star",
            Kind::Star => "Star",
            Kind::Dark => "Dark cloud",
            Kind::Cloud => "Star cloud",
            Kind::Asterism => "Asterism",
        };
        let pic = self.photos.credit(&piece.id).map(|c| c.id.clone());
        (kind.to_owned(), piece.name.clone(), all, pic)
    }

    fn star_detail(
        &self,
        hr: u16,
        year: i32,
    ) -> Option<(String, String, Vec<String>, Option<String>)> {
        let named = self.sky.lists.star_name(hr)?;
        let mut all: Vec<String> = named.describe(year).into_iter().collect();
        all.extend(named.more.iter().cloned());
        if let Some(star) = self.sky.stars.get(hr) {
            all.push(format!("Magnitude {:.1}.", star.mag));
        }
        let pic = self
            .photos
            .credit(&named.name.to_lowercase())
            .map(|c| c.id.clone());
        Some(("Star".to_owned(), named.name.clone(), all, pic))
    }

    /// Shows a subject's card, clear of where it was clicked.
    pub(crate) fn show_detail(&mut self, subject: Subject, at_x: f64, real: UnixMs) -> bool {
        let now = self.sky_now(real);
        let Some((kicker, title, body, picture)) = self.detail(subject, now) else {
            return false;
        };
        let w = self.camera.width;
        let list = if self.show_tonight() { 310.0 } else { 0.0 };
        // On the other side from what was clicked.
        let x = if at_x > w / 2.0 {
            24.0
        } else {
            (w - list - 424.0).max(24.0)
        };
        self.card = Some(Card {
            picture,
            find: match subject {
                Subject::Find(i) => Some(i),
                _ => None,
            },
            x,
            kicker,
            title,
            body,
            shown: real,
        });
        true
    }
}

impl Game {
    /// In free look the wisp drifts over, quietly, to keep the card
    /// company: no words, a slow flight, home again when the card goes.
    pub(crate) fn visit(&mut self, x: f64, y: f64, real: UnixMs) {
        if !self.calm() {
            self.fly(crate::guide::Aim::Spot(x, y), real, VISIT_MS);
        }
    }
}

/// How long the wisp stays beside a card in free look, at most.
const VISIT_MS: UnixMs = 8_000;

/// Adds where it is tonight to the kicker.
trait WithPlace {
    fn with_place(self, place: Option<String>) -> Self;
}

impl WithPlace for (String, String, Vec<String>, Option<String>) {
    fn with_place(self, place: Option<String>) -> Self {
        let (kicker, title, all, pic) = self;
        let kicker = match place {
            Some(p) => format!("{kicker} · {p}"),
            None => kicker,
        };
        (kicker, title, all, pic)
    }
}
