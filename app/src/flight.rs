//! How the wisp gets about: a springy chase towards wherever it's going, a
//! wide loop when it arrives at something in the sky to point it out, a gentle
//! bob while it hovers, and a trail of embers behind it.

use crate::view::Point;
use night_sky_core::time::UnixMs;

struct Ember {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    born: f64,
    life: f64,
    size: f32,
}

pub struct Flight {
    pub x: f64,
    pub y: f64,
    vx: f64,
    vy: f64,
    /// Where it's heading, and when it got there.
    goal: Option<(f64, f64)>,
    arrived: Option<f64>,
    /// A loop around what it's pointing at: centre, radius, start time.
    loop_round: Option<(f64, f64, f64, f64)>,
    embers: Vec<Ember>,
    last_ember: f64,
    seed: u64,
    pub home: bool,
}

const SPRING: f64 = 16.0;
const DAMPING: f64 = 6.2;
const TOP_SPEED: f64 = 1300.0;
const LOOP_MS: f64 = 1100.0;

impl Flight {
    pub fn new(x: f64, y: f64) -> Flight {
        Flight {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            goal: None,
            arrived: None,
            loop_round: None,
            embers: Vec::new(),
            last_ember: 0.0,
            seed: 0x2545_f491_4f6c_dd1d,
            home: true,
        }
    }

    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn speed(&self) -> f64 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }

    /// Leaning into the direction of travel, -1 to 1.
    pub fn lean(&self) -> f64 {
        (self.vx / 700.0).clamp(-1.0, 1.0)
    }

    /// Moves on by `dt` seconds towards `goal`, looping once around
    /// `pointing` the first time it arrives near it.
    pub fn step(
        &mut self,
        now: UnixMs,
        dt: f64,
        goal: (f64, f64),
        pointing: Option<(f64, f64)>,
        circle: bool,
        bob: bool,
    ) {
        let t = now as f64;
        if self.goal != Some(goal) {
            let moved = self
                .goal
                .is_none_or(|g| (g.0 - goal.0).abs() + (g.1 - goal.1).abs() > 40.0);
            if moved {
                self.arrived = None;
                self.loop_round = None;
            }
            self.goal = Some(goal);
        }
        let near = ((self.x - goal.0).powi(2) + (self.y - goal.1).powi(2)).sqrt() < 30.0;
        if near && self.arrived.is_none() {
            self.arrived = Some(t);
            if let Some((px, py)) = pointing.filter(|_| circle) {
                // Wide enough that the glow never covers what it's showing.
                let r = ((goal.0 - px).powi(2) + (goal.1 - py).powi(2))
                    .sqrt()
                    .clamp(100.0, 150.0);
                self.loop_round = Some((px, py, r, t));
            }
        }
        // Where it's aiming this moment: the goal, or a point on the loop.
        let (mut ax, mut ay) = goal;
        if let Some((cx, cy, r, start)) = self.loop_round {
            let f = (t - start) / LOOP_MS;
            if f < 1.0 {
                let begin = (goal.1 - cy).atan2(goal.0 - cx);
                let a = begin + f * std::f64::consts::TAU;
                ax = cx + r * a.cos();
                ay = cy + r * a.sin();
            } else {
                self.loop_round = None;
            }
        }
        if bob {
            ax += (t / 1700.0).sin() * 7.0;
            ay += (t / 1100.0).sin() * 6.0;
        }
        let (fx, fy) = (
            SPRING * (ax - self.x) - DAMPING * self.vx,
            SPRING * (ay - self.y) - DAMPING * self.vy,
        );
        self.vx += fx * dt;
        self.vy += fy * dt;
        let speed = self.speed();
        if speed > TOP_SPEED {
            self.vx *= TOP_SPEED / speed;
            self.vy *= TOP_SPEED / speed;
        }
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        // Embers: a stream while it flies, a spark now and then while it hovers.
        let speed = self.speed();
        let every = if speed > 60.0 { 14.0 } else { 260.0 };
        while t - self.last_ember > every {
            self.last_ember = if t - self.last_ember > 500.0 {
                t
            } else {
                self.last_ember + every
            };
            let (jx, jy) = (self.random() - 0.5, self.random() - 0.5);
            let life = 700.0 + self.random() * 600.0;
            let size = 1.3 + self.random() as f32 * 1.6;
            self.embers.push(Ember {
                x: self.x + jx * 10.0,
                y: self.y + 8.0 + jy * 8.0,
                dx: -self.vx * 0.08 + jx * 30.0,
                dy: -self.vy * 0.08 - 26.0 + jy * 20.0,
                born: t,
                life,
                size,
            });
        }
        for e in &mut self.embers {
            e.x += e.dx * dt;
            e.y += e.dy * dt;
            e.dx *= 1.0 - dt * 1.5;
        }
        self.embers.retain(|e| t - e.born < e.life);
    }

    /// Settles straight onto a spot, without flying there.
    pub fn place(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
        self.vx = 0.0;
        self.vy = 0.0;
    }

    /// The trail, as small glowing points.
    pub fn embers(&self, now: UnixMs, alpha: f32) -> Vec<Point> {
        let t = now as f64;
        self.embers
            .iter()
            .map(|e| {
                let f = ((t - e.born) / e.life).clamp(0.0, 1.0) as f32;
                // Gold to ember orange, dimming as it goes.
                let color = [1.0, 0.86 - 0.36 * f, 0.55 - 0.4 * f];
                Point {
                    x: e.x,
                    y: e.y,
                    radius: e.size * (1.0 - 0.6 * f),
                    color,
                    alpha: (1.0 - f) * 0.9 * alpha,
                    halo: 0.8 * (1.0 - f),
                }
            })
            .collect()
    }
}
