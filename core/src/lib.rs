//! Night Sky's core: the real sky over the user, the shape of a visit, and
//! the logbook, with no user interface and no clock of its own. Everything
//! that depends on time takes the time as an argument.

pub mod catalogues;
pub mod coords;
pub mod ephem;
pub mod events;
pub mod figures;
pub mod finale;
pub mod finds;
pub mod journal;
pub mod place;
pub mod session;
pub mod sky;
pub mod stars;
pub mod time;
