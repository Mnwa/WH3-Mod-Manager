use crate::{catalog::Source, storage::Settings};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

/// App manifests are a few kilobytes; a larger file is not a manifest worth parsing.
const MAX_MANIFEST: u64 = 1024 * 1024;

pub const APP_ID: &str = "1142710";

pub fn discover() -> Settings {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PROGRAMFILES(X86)") {
        candidates.push(PathBuf::from(path).join("Steam"));
    }
    if let Some(path) = std::env::var_os("PROGRAMFILES") {
        candidates.push(PathBuf::from(path).join("Steam"));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.extend([
            home.join("Library/Application Support/Steam"),
            home.join(".steam/steam"),
            home.join(".local/share/Steam"),
        ]);
    }
    for steam in candidates {
        for library in libraries(&steam) {
            let game = library
                .join("steamapps")
                .join("common")
                .join("Total War WARHAMMER III");
            if game.join("data").is_dir() {
                return for_game(game);
            }
        }
    }
    Settings::default()
}

pub fn for_game(game: PathBuf) -> Settings {
    let mut roots = vec![(game.join("data"), Source::Data)];
    if let Some(steamapps) = game.parent().and_then(Path::parent) {
        let workshop = steamapps.join("workshop").join("content").join(APP_ID);
        if workshop.is_dir() {
            roots.push((workshop, Source::Workshop));
        }
    }
    Settings {
        game_path: Some(game),
        roots,
        ..Settings::default()
    }
}

/// When Steam last updated the game, from `<steamapps>/appmanifest_1142710.acf`.
///
/// `game` is `<steamapps>/common/<game folder>`. Returns `None` for a game outside a Steam
/// library or an unreadable manifest, because the time only enables an advisory warning.
pub fn game_updated(game: &Path) -> Option<SystemTime> {
    let steamapps = game.parent()?.parent()?;
    let file = fs::File::open(steamapps.join(format!("appmanifest_{APP_ID}.acf"))).ok()?;
    let mut text = String::new();
    file.take(MAX_MANIFEST).read_to_string(&mut text).ok()?;
    manifest_updated(&text)
}

/// Parses the `LastUpdated` unix timestamp (seconds) of an app manifest.
pub fn manifest_updated(manifest: &str) -> Option<SystemTime> {
    let seconds = quoted_values(manifest, "LastUpdated")
        .first()?
        .trim()
        .parse::<u64>()
        .ok()?;
    // Zero means Steam never recorded an update. A huge value from a damaged file must not
    // panic on overflow.
    if seconds == 0 {
        return None;
    }
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(seconds))
}

pub fn libraries(steam: &Path) -> Vec<PathBuf> {
    let mut libraries = vec![steam.to_owned()];
    if let Ok(text) = fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
        for path in quoted_values(&text, "path") {
            let path = PathBuf::from(path);
            if !libraries.contains(&path) {
                libraries.push(path);
            }
        }
    }
    libraries
}

pub fn quoted_values(text: &str, key: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.clone().next() == Some('/') {
            for ch in chars.by_ref() {
                if ch == '\n' {
                    break;
                }
            }
        } else if ch == '"' {
            let mut token = String::new();
            while let Some(ch) = chars.next() {
                match ch {
                    '"' => break,
                    '\\' => {
                        if let Some(escaped) = chars.next() {
                            token.push(escaped);
                        }
                    }
                    _ => token.push(ch),
                }
            }
            tokens.push(token);
        } else if ch == '{' || ch == '}' {
            tokens.push(ch.to_string());
        }
    }
    tokens
        .windows(2)
        .filter(|pair| pair[0] == key && pair[1] != "{")
        .map(|pair| pair[1].clone())
        .collect()
}
