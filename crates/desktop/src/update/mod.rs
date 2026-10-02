//! Self-update from GitHub Releases: a background check, verified install and a
//! restart request that the window owner handles after saving its state.
mod model;
mod view;

pub use model::{Phase, Updater, UpdaterEvent};
pub use view::update_button;
