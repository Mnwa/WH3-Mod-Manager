//! Steam Workshop operations through a short-lived worker process.
//!
//! Initialising Steamworks with the game's app id makes Steam treat the caller
//! as the running game, which can block launching the real game. Like the
//! original manager, every operation therefore runs in a separate process
//! (`<exe> --steam-worker`) that reads one [`Request`] from stdin, writes one
//! [`Response`] to stdout and exits. The GUI process never loads
//! `steam_api64.dll`; the worker finds it in its working directory.
mod client;
pub mod merge;
#[cfg(all(windows, feature = "steam"))]
mod worker;

pub use client::{Runtime, call};
#[cfg(all(windows, feature = "steam"))]
pub use worker::serve;

use serde::{Deserialize, Serialize};

/// Hidden command-line flag that turns the executable into the worker.
pub const WORKER_FLAG: &str = "--steam-worker";
/// Steamworks itself prints diagnostics to stdout, so the answer follows this marker.
pub const RESPONSE_MARKER: &[u8] = b"\n@@wh3mm-response@@\n";
/// `EItemState` bits used by the manager.
pub const STATE_SUBSCRIBED: u32 = 1;
pub const STATE_INSTALLED: u32 = 4;
pub const STATE_NEEDS_UPDATE: u32 = 8;
pub const STATE_DOWNLOADING: u32 = 16;
pub const STATE_DOWNLOAD_PENDING: u32 = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Titles, authors, update times, tags and required items.
    Details {
        ids: Vec<u64>,
    },
    /// Local install state only; cheap enough to poll while downloads run.
    State {
        ids: Vec<u64>,
    },
    Subscribed,
    Subscribe {
        ids: Vec<u64>,
    },
    Unsubscribe {
        ids: Vec<u64>,
    },
    /// Ask Steam to (re)download items with high priority.
    Download {
        ids: Vec<u64>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub title: String,
    pub author: String,
    /// Pack file name as published, e.g. `my_mod.pack`.
    pub file_name: String,
    /// Unix seconds of the last Workshop update.
    pub time_updated: u32,
    pub tags: Vec<String>,
    /// Required Workshop items ("children" in the Steam API).
    pub required: Vec<u64>,
    pub banned: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub id: u64,
    pub state: u32,
    /// Unix seconds when the installed copy was written, if installed.
    pub installed: Option<u32>,
    pub downloaded: u64,
    pub total: u64,
}

impl State {
    pub fn has(&self, bit: u32) -> bool {
        self.state & bit != 0
    }

    pub fn busy(&self) -> bool {
        self.has(STATE_DOWNLOADING | STATE_DOWNLOAD_PENDING)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    Details(Vec<Item>),
    States(Vec<State>),
    Subscribed(Vec<u64>),
    /// Per-item results of subscribe/unsubscribe/download; errors keep Steam's text.
    Done {
        accepted: Vec<u64>,
        failed: Vec<(u64, String)>,
    },
    Failed(String),
}

/// An item needs an update when Workshop has a newer revision than the install,
/// mirroring the original's "Workshop mods may be outdated" check.
pub fn needs_update(item: &Item, state: &State) -> bool {
    state.has(STATE_NEEDS_UPDATE)
        || state
            .installed
            .is_some_and(|installed| item.time_updated > installed)
}

/// Accept `123`, a Workshop/collection URL or `?id=123` forms.
pub fn parse_id(text: &str) -> Option<u64> {
    let text = text.trim();
    let digits = match text.find("id=") {
        Some(start) => &text[start + 3..],
        None => text,
    };
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok().filter(|&id| id != 0)
}
