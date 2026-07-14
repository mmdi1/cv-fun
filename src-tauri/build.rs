fn main() {
    // IMPORTANT: build.rs always runs on the *host*. Do not use
    // `cfg(target_os = "macos")` here — that is true when cross-compiling
    // Windows from a Mac and would feed `-fobjc-arc` to mingw-gcc.
    // Use CARGO_CFG_TARGET_OS for the crate being built instead.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        // Align ObjC min version with Tauri / typical MACOSX_DEPLOYMENT_TARGET (11.0).
        // Do not use @available in .m (emits ___isPlatformVersionAtLeast; rustc link fails).
        let min = std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".into());
        cc::Build::new()
            .file("native/macos_ocr.m")
            .flag("-fobjc-arc")
            .flag(format!("-mmacosx-version-min={min}"))
            .compile("nfun_macos_ocr");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Vision");
        println!("cargo:rerun-if-changed=native/macos_ocr.m");
        println!("cargo:rerun-if-changed=native/macos_ocr.h");
    }
    tauri_build::build()
}
