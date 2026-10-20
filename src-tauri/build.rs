fn main() {
    // The dialog plugin needs Common Controls v6, which the app manifest asks for. Tauri embeds that manifest in
    // the app only, so on Windows the test executables couldn't start (STATUS_ENTRYPOINT_NOT_FOUND); the linker
    // embeds it in every executable instead.
    let mut windows = tauri_build::WindowsAttributes::new();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    }
    // ggml's Metal code checks `@available(macOS 15)`. Below that deployment target the check runs at launch through
    // clang's runtime (`__isPlatformVersionAtLeast`), which rustc doesn't link (-nodefaultlibs).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
        && std::env::var_os("CARGO_FEATURE_LOCAL_MODEL").is_some()
    {
        let out = std::process::Command::new("clang")
            .arg("--print-resource-dir")
            .output()
            .expect("clang is needed to build llama.cpp");
        let resources = String::from_utf8(out.stdout).unwrap();
        println!(
            "cargo:rustc-link-arg={}/lib/darwin/libclang_rt.osx.a",
            resources.trim()
        );
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run the Tauri build script");
}
