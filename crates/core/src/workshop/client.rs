use super::{Request, Response, WORKER_FLAG};
use crate::{Error, Result, error::io};
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// The worker is this executable; `steam_api64.dll` ships next to it in the
/// release archive and is resolved from the executable's folder.
#[derive(Clone)]
pub struct Runtime {
    pub exe: PathBuf,
}

impl Runtime {
    pub fn current() -> Result<Self> {
        std::env::current_exe()
            .map(|exe| Self { exe })
            .map_err(|e| io(std::path::Path::new("."), e))
    }

    fn dir(&self) -> Result<&std::path::Path> {
        self.exe.parent().ok_or_else(unavailable)
    }
}

/// Responses are small JSON documents; cap them so a broken worker cannot exhaust memory.
const MAX_RESPONSE: u64 = 32 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(120);

fn unavailable() -> Error {
    Error::Invalid(crate::message!(
        "Steam features need the Windows build with Steam running",
        "Функции Steam доступны в Windows-сборке при запущенном Steam"
    ))
}

/// Run one request in a fresh worker process and wait for its answer.
pub fn call(runtime: &Runtime, request: &Request) -> Result<Response> {
    if !cfg!(all(windows, feature = "steam")) {
        return Err(unavailable());
    }
    let dir = runtime.dir()?;
    if !dir.join("steam_api64.dll").is_file() {
        return Err(Error::Invalid(crate::message!(
            "steam_api64.dll is missing next to the manager. Unpack the whole release archive.",
            "Рядом с менеджером нет steam_api64.dll. Распакуйте архив релиза целиком."
        )));
    }
    let mut command = Command::new(&runtime.exe);
    command
        .arg(WORKER_FLAG)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the worker is invisible plumbing.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| io(&runtime.exe, e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&serde_json::to_vec(request)?)
            .map_err(|e| io(&runtime.exe, e))?;
    }
    let mut stdout = child.stdout.take().ok_or_else(unavailable)?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = (&mut stdout).take(MAX_RESPONSE).read_to_end(&mut bytes);
        result.map(|_| bytes)
    });
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|e| io(&runtime.exe, e))? {
            Some(_) => break,
            None if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                return Err(Error::Invalid(crate::message!(
                    "Steam did not answer in time",
                    "Steam не ответил вовремя"
                )));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let bytes = reader
        .join()
        .map_err(|_| unavailable())?
        .map_err(|e| io(&runtime.exe, e))?;
    let marker = super::RESPONSE_MARKER;
    let start = bytes
        .windows(marker.len())
        .rposition(|window| window == marker)
        .map(|position| position + marker.len())
        .ok_or_else(unavailable)?;
    match serde_json::from_slice(&bytes[start..])? {
        Response::Failed(message) => Err(Error::Steam(message)),
        response => Ok(response),
    }
}
