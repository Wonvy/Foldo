fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    // unrar_sys 0.5.8 uses Windows registry, token and CryptoAPI functions,
    // but does not link their import library. Keep this with foldo-core so
    // its tests, the CLI and the desktop app all receive the missing library.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo::rustc-link-lib=advapi32");
    }
}
