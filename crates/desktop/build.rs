//! Delay-load `steam_api64.dll`: the GUI never calls Steamworks itself (the
//! `--steam-worker` child does), so the executable must start even when the DLL
//! is not next to it.
fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows-msvc") {
        println!("cargo:rustc-link-arg-bins=/DELAYLOAD:steam_api64.dll");
        println!("cargo:rustc-link-arg-bins=delayimp.lib");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
