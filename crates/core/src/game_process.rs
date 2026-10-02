//! Whether Total War: WARHAMMER III is running, and closing it on request.
//!
//! Like the original manager this asks Windows' own `tasklist`/`taskkill`, which needs
//! no extra privileges or unsafe process APIs. Both block for a few tens of
//! milliseconds; call them from background tasks only.
use crate::Result;

pub const PROCESS_NAME: &str = "Warhammer3.exe";

/// True when `tasklist` output lists the game; matching ignores case like Windows does.
pub fn lists_game(tasklist_output: &str) -> bool {
    let name = PROCESS_NAME.to_ascii_lowercase();
    tasklist_output
        .lines()
        .any(|line| line.trim_start().to_ascii_lowercase().starts_with(&name))
}

#[cfg(target_os = "windows")]
fn command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: a polling GUI must not flash console windows.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = std::process::Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(target_os = "windows")]
pub fn running() -> Result<bool> {
    let output = command("tasklist")
        .args(["/NH", "/FI", &format!("IMAGENAME eq {PROCESS_NAME}")])
        .output()
        .map_err(|error| crate::error::io(std::path::Path::new("tasklist"), error))?;
    Ok(lists_game(&String::from_utf8_lossy(&output.stdout)))
}

/// Ends the game and its child processes, as the original's Ctrl+click on Play.
#[cfg(target_os = "windows")]
pub fn close() -> Result<()> {
    let output = command("taskkill")
        .args(["/F", "/T", "/IM", PROCESS_NAME])
        .output()
        .map_err(|error| crate::error::io(std::path::Path::new("taskkill"), error))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(crate::Error::Invalid(crate::message!(
            "Could not close the game: {}",
            "Не удалось закрыть игру: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// Set the running game's priority class to High, like the original's "Change game
/// process priority". Windows may refuse; the caller reports that once.
#[cfg(target_os = "windows")]
pub fn raise_priority() -> Result<()> {
    let name = PROCESS_NAME.trim_end_matches(".exe");
    let output = command("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("Get-Process -Name '{name}' | ForEach-Object {{ $_.PriorityClass = 'High' }}"),
        ])
        .output()
        .map_err(|error| crate::error::io(std::path::Path::new("powershell.exe"), error))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() && stderr.trim().is_empty() {
        Ok(())
    } else {
        Err(crate::Error::Invalid(crate::message!(
            "Could not raise the game's process priority: {}",
            "Не удалось повысить приоритет процесса игры: {}",
            stderr.trim()
        )))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn raise_priority() -> Result<()> {
    Ok(())
}

/// The game only runs on Windows here; other systems never report it.
#[cfg(not(target_os = "windows"))]
pub fn running() -> Result<bool> {
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
pub fn close() -> Result<()> {
    Ok(())
}
