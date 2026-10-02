use crate::{catalog::Source, storage::Settings};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
            let game = library.join("steamapps/common/Total War WARHAMMER III");
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
        let workshop = steamapps.join("workshop/content").join(APP_ID);
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

pub fn libraries(steam: &Path) -> Vec<PathBuf> {
    let mut libraries = vec![steam.to_owned()];
    if let Ok(text) = fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
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
