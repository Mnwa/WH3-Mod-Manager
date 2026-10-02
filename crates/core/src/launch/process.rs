use super::MOD_LIST;
use crate::{Error, Result};
use std::path::Path;

/// Arguments for a direct `Warhammer3.exe` spawn, matching the original
/// manager's `buildGameArgs`. Each value stays a separate argv entry because the
/// semicolons are game arguments rather than shell separators.
pub fn arguments(save: Option<&str>) -> Vec<String> {
    let mut arguments = Vec::with_capacity(5);
    // The original manager treats an empty save name as "no save".
    if let Some(save) = save.filter(|save| !save.is_empty()) {
        arguments.extend(
            ["game_startup_mode", "campaign_load", save, ";"]
                .into_iter()
                .map(String::from),
        );
    }
    arguments.push(format!("{MOD_LIST};"));
    arguments
}

/// The game splits its command line on `;` and reads the save from its own
/// save folder, so separators or paths in a name would load something else.
fn check_save(save: &str) -> Result<()> {
    if save.contains([';', '"', '/', '\\']) || save.chars().any(char::is_control) {
        return Err(Error::Invalid(crate::message!(
            "Invalid characters in save name: {}",
            "Недопустимые символы в имени сохранения: {}",
            save
        )));
    }
    Ok(())
}

/// Starts the game with the prepared mod list, optionally loading a campaign save.
pub fn start(game: &Path, save: Option<&str>) -> Result<()> {
    if let Some(save) = save {
        check_save(save)?;
    }
    spawn(game, &arguments(save))
}

#[cfg(target_os = "windows")]
fn spawn(game: &Path, arguments: &[String]) -> Result<()> {
    use std::os::windows::process::CommandExt;
    // DETACHED_PROCESS: the game must outlive the manager and needs no console.
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let mut child = std::process::Command::new(game.join("Warhammer3.exe"))
        .current_dir(game)
        .args(arguments)
        .creation_flags(DETACHED_PROCESS)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| crate::error::io(game, e))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn(_: &Path, _: &[String]) -> Result<()> {
    Err(Error::Invalid(crate::message!(
        "Launching the game requires the Windows build",
        "Запуск игры доступен в Windows-сборке"
    )))
}
