//! Консольный экспортёр сохранённых данных оригинального менеджера.
use std::path::Path;

fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 || args[0] != "export" {
        eprintln!("Usage: wh3-meta export <original-config.json> <wh3-metadata.json>");
        return std::process::ExitCode::from(2);
    }
    match wh3_core::metadata::export(Path::new(&args[1]), Path::new(&args[2])) {
        Ok(bundle) => {
            println!(
                "Exported: {} mods, {} presets",
                bundle.mods.len(),
                bundle.presets.len()
            );
            for warning in bundle.warnings {
                println!("{warning}");
            }
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
