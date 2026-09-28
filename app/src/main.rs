//! Night Sky: the real sky over you tonight, a few quiet questions, and then
//! the real sky outside.

mod book;
mod camera;
mod course;
mod drawing;
mod eyepiece;
mod field;
mod flight;
mod game;
mod guide;
mod hud;
mod music;
mod settings;
mod sprite;
mod talk;
mod ui;
mod view;

use book::Book;
use course::Course;
use game::{Clock, Game, Options};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use night_sky_core::coords::Observer;
use night_sky_core::journal::Journal;
use night_sky_core::place;
use night_sky_core::session::Timings;
use night_sky_core::time::UnixMs;
use settings::Settings;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use talk::Request;
use ui::PromptBar;
use view::SkyView;

const APP_ID: &str = "io.github.peterwalker78.NightSky";

/// Options for trying the app out: `--at=2026-12-14T21:00` starts the sky at
/// another moment, `--speed=N` runs it N times faster, `--quick=N` shortens
/// the visit's own timings N times, `--data=DIR` keeps everything in DIR and
/// `--place=LAT,LON` stands somewhere else, and `--profile` prints what
/// frames cost.
#[derive(Default)]
struct Args {
    at: Option<String>,
    speed: Option<f64>,
    quick: Option<i64>,
    data: Option<PathBuf>,
    place: Option<(f64, f64)>,
    /// Print how long frames take to make, every few seconds.
    profile: bool,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    for arg in std::env::args().skip(1) {
        let (key, value) = arg.split_once('=').unwrap_or((arg.as_str(), ""));
        match key {
            "--at" => args.at = Some(value.to_owned()),
            "--speed" => args.speed = value.parse().ok(),
            "--quick" => args.quick = value.parse().ok(),
            "--data" => args.data = Some(PathBuf::from(value)),
            "--profile" => args.profile = true,
            "--place" => {
                args.place = value
                    .split_once(',')
                    .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
            }
            _ => eprintln!("night-sky: ignoring {arg}"),
        }
    }
    args
}

pub fn wall_clock() -> UnixMs {
    glib::real_time() / 1000
}

fn utc_offset(at: UnixMs) -> i32 {
    glib::DateTime::from_unix_local(at / 1000)
        .map(|t| t.utc_offset().as_seconds() as i32)
        .unwrap_or(0)
}

fn parse_moment(text: &str) -> Option<UnixMs> {
    let local = glib::TimeZone::local();
    let with_seconds = if text.len() == 16 {
        format!("{text}:00")
    } else {
        text.to_owned()
    };
    glib::DateTime::from_iso8601(&with_seconds, Some(&local))
        .ok()
        .map(|t| t.to_unix() * 1000)
}

fn data_dir(args: &Args) -> PathBuf {
    args.data
        .clone()
        .unwrap_or_else(|| glib::user_data_dir().join("night-sky"))
}

fn build(app: &gtk::Application, args: &Rc<Args>) {
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_interface_color_scheme(gtk::InterfaceColorScheme::Dark);
    }
    let real = wall_clock();
    let sky0 = args.at.as_deref().and_then(parse_moment).unwrap_or(real);
    let journal = Journal::open(&data_dir(args));
    let observer = args
        .place
        .map(|(lat, lon)| Observer { lat, lon })
        .or_else(|| {
            let s = &journal.settings;
            Some(Observer {
                lat: s.lat?,
                lon: s.lon?,
            })
        })
        .or_else(|| place::here(glib::TimeZone::local().identifier().as_str().into()))
        .unwrap_or(Observer {
            lat: 51.48,
            lon: 0.0,
        });
    let timings = args.quick.map(Timings::quick).unwrap_or(Timings::STANDARD);
    let options = Options {
        clock: Clock {
            real0: real,
            sky0,
            speed: args.speed.unwrap_or(1.0),
        },
        observer,
        offset_s: utc_offset(sky0),
        timings,
        journal,
    };
    let game = Rc::new(RefCell::new(Game::new(options, real)));

    ui::install_css();
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Night Sky")
        .default_width(1280)
        .default_height(800)
        .decorated(false)
        .build();
    window.maximize();

    let view = SkyView::default();
    view.set_hexpand(true);
    view.set_vexpand(true);
    view.set_focusable(true);
    view.set_cursor_from_name(Some("crosshair"));
    let prompt = PromptBar::new(&game);
    let sky_page = gtk::Overlay::new();
    sky_page.set_child(Some(&view));
    sky_page.add_overlay(&prompt.root);
    let tonight = hud::Tonight::new();
    sky_page.add_overlay(&tonight.root);

    let book = Book::new(&game);
    let course = Course::new(&game);
    let settings = Settings::new(&game);
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(350);
    stack.add_named(&sky_page, Some("sky"));
    stack.add_named(&book.root, Some("book"));
    stack.add_named(&course.root, Some("course"));
    stack.add_named(&settings.root, Some("settings"));
    window.set_child(Some(&stack));

    // Going from page to page. The pages ask for this with a Request.
    let go: Rc<dyn Fn(Option<Request>)> = {
        let (stack, view) = (stack.clone(), view.clone());
        let (book, course, settings) = (book.clone(), course.clone(), settings.clone());
        Rc::new(move |r: Option<Request>| match r {
            None => {
                stack.set_visible_child_name("sky");
                view.grab_focus();
            }
            Some(Request::Book(at)) => {
                stack.set_visible_child_name("book");
                book.open(at.as_deref());
            }
            Some(Request::Course(weight)) => {
                stack.set_visible_child_name("course");
                course.open(weight);
            }
            Some(Request::Settings) => {
                stack.set_visible_child_name("settings");
                settings.open();
            }
        })
    };
    for slot in [&book.on_request, &course.on_request, &settings.on_request] {
        let go = go.clone();
        *slot.borrow_mut() = Some(Box::new(move |r| go(r)));
    }

    // Keys reach the sky before GTK's own arrow-key focus navigation can
    // claim them; anything the sky doesn't want goes on to the focused widget.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let game = game.clone();
        let window = window.clone();
        let stack = stack.clone();
        let go = go.clone();
        keys.connect_key_pressed(move |_, key, _, state| {
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            if ctrl && matches!(key, gdk::Key::q | gdk::Key::w) {
                window.close();
                return glib::Propagation::Stop;
            }
            if ctrl && key == gdk::Key::comma {
                go(Some(Request::Settings));
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::F11 {
                if window.is_fullscreen() {
                    window.unfullscreen();
                } else {
                    window.fullscreen();
                }
                return glib::Propagation::Stop;
            }
            if stack.visible_child_name().as_deref() != Some("sky") {
                return glib::Propagation::Proceed;
            }
            let handled = game.borrow_mut().key_pressed(key, wall_clock());
            let request = game.borrow_mut().take_request();
            if request.is_some() {
                go(request);
            }
            if handled {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }
    {
        let game = game.clone();
        keys.connect_key_released(move |_, key, _, _| {
            game.borrow_mut().key_released(key, wall_clock());
        });
    }
    window.add_controller(keys);

    let drag = gtk::GestureDrag::new();
    {
        let game = game.clone();
        drag.connect_drag_begin(move |_, _, _| game.borrow_mut().drag_begin(wall_clock()));
    }
    {
        let game = game.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            game.borrow_mut().drag_update(dx, dy, wall_clock())
        });
    }
    {
        let game = game.clone();
        drag.connect_drag_end(move |gesture, dx, dy| {
            if dx.abs() < 3.0
                && dy.abs() < 3.0
                && let Some((x, y)) = gesture.start_point()
            {
                game.borrow_mut().click(x, y, wall_clock());
            }
            game.borrow_mut().drag_end();
        });
    }
    view.add_controller(drag);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    {
        let game = game.clone();
        scroll.connect_scroll(move |_, _, dy| {
            game.borrow_mut().scroll(dy, wall_clock());
            glib::Propagation::Stop
        });
    }
    view.add_controller(scroll);

    let music = Rc::new(RefCell::new(music::Music::new(
        real as u64 / 86_400_000,
        game.borrow().quiet(),
    )));
    let last_frame = std::cell::Cell::new(0i64);
    let last_music = std::cell::Cell::new(real);
    let last_list = std::cell::Cell::new(0i64);
    let spent = std::cell::Cell::new((0.0f64, 0u32, real));
    let profile = args.profile;
    {
        let game = game.clone();
        let window = window.clone();
        let stack = stack.clone();
        let go = go.clone();
        view.add_tick_callback(move |view, _clock| {
            let real = wall_clock();
            let fast = game.borrow().wants_fast_frames(real);
            let on_sky = stack.visible_child_name().as_deref() == Some("sky");
            let interval = if !window.is_active() || !on_sky {
                200
            } else if fast {
                16
            } else {
                // At rest the sky only twinkles, slowly.
                66
            };
            if real - last_frame.get() >= interval {
                last_frame.set(real);
                let mut g = game.borrow_mut();
                g.resize(
                    view.width() as f64,
                    view.height() as f64,
                    view.scale_factor() as f64,
                );
                let began = std::time::Instant::now();
                let frame = g.tick(real);
                if profile {
                    let (sum, n, since) = spent.get();
                    let sum = sum + began.elapsed().as_secs_f64() * 1000.0;
                    if real - since > 5_000 {
                        eprintln!(
                            "night-sky: {:.2} ms a frame, {:.1} frames a second",
                            sum / (n + 1) as f64,
                            (n + 1) as f64 / ((real - since) as f64 / 1000.0)
                        );
                        spent.set((0.0, 0, real));
                    } else {
                        spent.set((sum, n + 1, since));
                    }
                }
                let quit = g.quit;
                let request = g.take_request();
                let (level, quiet) = (g.music_level(real), g.quiet());
                drop(g);
                {
                    let mut m = music.borrow_mut();
                    m.quiet = quiet;
                    let dt = (real - last_music.get()).clamp(0, 500) as f64 / 1000.0;
                    last_music.set(real);
                    m.tick(level, dt);
                }
                view.show(frame);
                if on_sky && real - last_list.get() > 700 {
                    last_list.set(real);
                    tonight.sync(&game);
                }
                if prompt.sync(&game) && on_sky {
                    view.grab_focus();
                }
                if request.is_some() {
                    go(request);
                }
                if quit {
                    window.close();
                }
            }
            glib::ControlFlow::Continue
        });
    }

    window.present();
    view.grab_focus();
}

fn main() -> glib::ExitCode {
    let args = Rc::new(parse_args());
    let mut flags = gio::ApplicationFlags::empty();
    if args.data.is_some() || args.at.is_some() {
        flags |= gio::ApplicationFlags::NON_UNIQUE;
    }
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(flags)
        .build();
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
        } else {
            build(app, &args);
        }
    });
    let program: Vec<String> = std::env::args().take(1).collect();
    app.run_with_args(&program)
}
