//! Self-update from the GitHub Releases that `release.yml` publishes.
//!
//! Every call blocks on the network, so callers run it on a background thread.
//! The release workflow attaches the executable and `SHA256SUMS.txt` after the
//! release is published; until both exist the release is not offered.
use crate::{Error, Result};
use self_update::{Release, ReleaseAsset, ReleaseStatus, backends::github};
use std::time::Duration;

pub const REPO_OWNER: &str = "Mnwa";
pub const REPO_NAME: &str = "WH3-Mod-Manager";
pub const EXE_ASSET: &str = "wh3-mod-manager.exe";
pub const SUMS_ASSET: &str = "SHA256SUMS.txt";
/// Steamworks redistributable shipped next to the executable for the Workshop worker.
pub const STEAM_API_ASSET: &str = "steam_api64.dll";
pub const RELEASES_URL: &str = "https://github.com/Mnwa/WH3-Mod-Manager/releases";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Available {
    pub version: String,
    pub notes_url: Option<String>,
}

fn failure(error: self_update::errors::Error) -> Error {
    Error::Update(error.to_string())
}

fn updater(current: &str) -> Result<github::Update> {
    github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("wh3-mod-manager")
        .current_version(current)
        .asset_matcher(|assets: &[ReleaseAsset]| {
            assets
                .iter()
                .find(|asset| asset.name() == EXE_ASSET)
                .cloned()
        })
        .checksum_from_asset(SUMS_ASSET)
        .timeout(Duration::from_secs(60))
        .unattended()
        .build()
        .map_err(failure)
}

/// Decide whether `release` should be offered without touching the network.
pub fn newer(current: &str, release: &Release) -> Option<Available> {
    let version = release.version().trim_start_matches('v');
    let has = |name: &str| release.assets().iter().any(|asset| asset.name() == name);
    let greater = self_update::version::bump_is_greater(current, version).unwrap_or(false);
    (greater && has(EXE_ASSET) && has(SUMS_ASSET)).then(|| Available {
        version: version.to_owned(),
        notes_url: release.release_notes_url().map(str::to_owned),
    })
}

/// Return the latest published release when it is newer than `current`.
pub fn check(current: &str) -> Result<Option<Available>> {
    let releases = updater(current)?.get_latest_release().map_err(failure)?;
    Ok(releases
        .latest()
        .and_then(|release| newer(current, release)))
}

/// Download the newest executable, verify it against `SHA256SUMS.txt` and
/// replace the running binary. The new version starts after a relaunch.
pub fn install(current: &str) -> Result<String> {
    if !cfg!(windows) {
        return Err(Error::Invalid(crate::message!(
            "Updates are published only for the Windows build",
            "Обновления публикуются только для Windows-сборки"
        )));
    }
    match updater(current)?.update_extended().map_err(failure)? {
        ReleaseStatus::Updated(release) => {
            refresh_steam_api(&release)?;
            Ok(release.version().trim_start_matches('v').into())
        }
        _ => Err(Error::Invalid(crate::message!(
            "The latest version is already installed",
            "Уже установлена последняя версия"
        ))),
    }
}

fn download(asset: &ReleaseAsset, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    // `download_url` is the API address; GitHub serves the file only with this Accept header.
    self_update::Download::from_url(asset.download_url())
        .request_header("Accept", "application/octet-stream")
        .max_download_size(limit)
        .timeout(Duration::from_secs(60))
        .download_to(&mut bytes)
        .map_err(failure)?;
    Ok(bytes)
}

/// Replace `steam_api64.dll` next to the executable when the release ships a
/// different one. The GUI never loads it (only the worker does), so it is free
/// to be replaced while the manager runs.
fn refresh_steam_api(release: &Release) -> Result<()> {
    use sha2::Digest;
    let asset = |name: &str| release.assets().iter().find(|asset| asset.name() == name);
    let (Some(dll), Some(sums)) = (asset(STEAM_API_ASSET), asset(SUMS_ASSET)) else {
        return Ok(());
    };
    let sums = String::from_utf8_lossy(&download(sums, 1024 * 1024)?).into_owned();
    let expected = sums
        .lines()
        .filter_map(|line| line.split_once(char::is_whitespace))
        .find(|(_, name)| name.trim().trim_start_matches('*') == STEAM_API_ASSET)
        .map(|(hash, _)| hash.trim().to_ascii_lowercase())
        .ok_or_else(|| Error::Update(format!("{SUMS_ASSET} has no entry for {STEAM_API_ASSET}")))?;
    let exe =
        std::env::current_exe().map_err(|e| crate::error::io(std::path::Path::new("."), e))?;
    let path = exe.with_file_name(STEAM_API_ASSET);
    let hex = |bytes: &[u8]| -> String {
        sha2::Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    if std::fs::read(&path).is_ok_and(|current| hex(&current) == expected) {
        return Ok(());
    }
    let bytes = download(dll, 32 * 1024 * 1024)?;
    if hex(&bytes) != expected {
        return Err(Error::Update(format!(
            "{STEAM_API_ASSET} checksum mismatch"
        )));
    }
    crate::storage::atomic_write(&path, &bytes)
}

/// Start the replaced executable with the current arguments; the caller then quits.
pub fn relaunch() -> Result<()> {
    let exe =
        std::env::current_exe().map_err(|e| crate::error::io(std::path::Path::new("."), e))?;
    std::process::Command::new(&exe)
        .args(std::env::args_os().skip(1))
        .spawn()
        .map(drop)
        .map_err(|e| crate::error::io(&exe, e))
}
