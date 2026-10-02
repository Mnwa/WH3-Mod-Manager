#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

// Scanning and conflict checks allocate many short-lived strings across worker threads.
#[global_allocator]
static GLOBAL: mimalloc3::MiMalloc = mimalloc3::MiMalloc;

fn main() -> std::process::ExitCode {
    // The same executable serves short-lived Steam Workshop requests.
    #[cfg(windows)]
    if std::env::args().any(|arg| arg == wh3_core::workshop::WORKER_FLAG) {
        return wh3_core::workshop::serve();
    }
    wh3_mod_manager::run();
    std::process::ExitCode::SUCCESS
}
