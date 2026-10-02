//! Command-line exporter for the original manager's persisted metadata.
use std::path::Path;
use wh3_core::localization::Language;

fn main() -> std::process::ExitCode {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let language = match args.first().and_then(|arg| arg.to_str()) {
        Some("--lang=ru") => {
            args.remove(0);
            Language::Russian
        }
        Some("--lang=en") => {
            args.remove(0);
            Language::English
        }
        _ => Language::English,
    };
    if args.len() != 3 || args[0] != "export" {
        eprintln!("{}", language.text(
            "Usage: wh3-meta [--lang=en|ru] export <original-config.json> <wh3-metadata.json>",
            "Использование: wh3-meta [--lang=en|ru] export <original-config.json> <wh3-metadata.json>",
        ));
        return std::process::ExitCode::from(2);
    }
    match wh3_core::metadata::export(Path::new(&args[1]), Path::new(&args[2])) {
        Ok(bundle) => {
            println!(
                "{}",
                wh3_core::message!(
                    "Exported: {} mods, {} presets",
                    "Экспортировано: {} модов, {} пресетов",
                    bundle.mods.len(),
                    bundle.presets.len(),
                )
                .text(language)
            );
            for warning in bundle.warnings {
                println!(
                    "{}",
                    wh3_core::metadata::warning_message(warning).text(language)
                );
            }
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", error.message().text(language));
            std::process::ExitCode::FAILURE
        }
    }
}
