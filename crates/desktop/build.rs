//! Delay-load `steam_api64.dll`: the GUI never calls Steamworks itself (the
//! `--steam-worker` child does), so the executable must start even when the DLL
//! is not next to it. Windows builds also embed the icon and version information.
fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows-msvc") {
        println!("cargo:rustc-link-arg-bins=/DELAYLOAD:steam_api64.dll");
        println!("cargo:rustc-link-arg-bins=delayimp.lib");
        embed_windows_resources();
    }
    println!("cargo:rerun-if-changed=build.rs");
}

fn embed_windows_resources() {
    for path in ["assets/windows/app.rc", "assets/app.ico"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let version = |name: &str| std::env::var(name).unwrap_or_else(|_| "0".into());
    let header = format!(
        "#define VERSION_MAJOR {}\n#define VERSION_MINOR {}\n#define VERSION_PATCH {}\n#define VERSION_STRING \"{}\"\n",
        version("CARGO_PKG_VERSION_MAJOR"),
        version("CARGO_PKG_VERSION_MINOR"),
        version("CARGO_PKG_VERSION_PATCH"),
        version("CARGO_PKG_VERSION"),
    );
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    if let Err(error) = std::fs::write(out.join("version.h"), header) {
        panic!("cannot write version.h: {error}");
    }
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let assets = std::path::Path::new(&manifest).join("assets");
    let includes = [out.into_os_string(), assets.into_os_string()];
    let no_macros: [&str; 0] = [];
    if let Err(error) = embed_resource::compile(
        "assets/windows/app.rc",
        embed_resource::ParamsMacrosAndIncludeDirs(no_macros, includes),
    )
    .manifest_optional()
    {
        panic!("cannot embed the Windows icon and version information: {error}");
    }
}
