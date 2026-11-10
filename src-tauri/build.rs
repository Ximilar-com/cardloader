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
    gather_llama_libs();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run the Tauri build script");
}

/// With `gpu-backends`, llama.cpp is shared libraries plus backend modules (Vulkan, CUDA, CPU variants) that it
/// loads at runtime. They are gathered into `llama/`, which the bundles ship: next to the exe on Windows
/// (`tauri.windows.conf.json`), in `<resources>/llama` on Linux (`tauri.linux.conf.json`), where the binary's rpath
/// finds them. The folder always exists, as Tauri checks bundled paths while building; without the feature it is
/// empty, and Tauri skips an empty folder.
fn gather_llama_libs() {
    use std::path::{Path, PathBuf};
    let dir = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("llama");
    std::fs::create_dir_all(&dir).expect("create src-tauri/llama");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let windows = os == "windows";
    let mut wanted: Vec<PathBuf> = Vec::new();
    if std::env::var_os("CARGO_FEATURE_GPU_BACKENDS").is_some() && os != "macos" {
        println!("cargo:rerun-if-env-changed=DEP_LLAMA_BACKENDS_DIR");
        let backends = PathBuf::from(
            std::env::var("DEP_LLAMA_BACKENDS_DIR")
                .expect("llama-cpp-sys-2 with dynamic-backends exports DEP_LLAMA_BACKENDS_DIR"),
        );
        let out = backends.parent().unwrap().to_path_buf();
        let files = |dir: &Path, keep: &dyn Fn(&str) -> bool| -> Vec<PathBuf> {
            std::fs::read_dir(dir)
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(keep))
                        .collect()
                })
                .unwrap_or_default()
        };
        if windows {
            wanted.extend(files(&backends, &|n| n.ends_with(".dll")));
            wanted.extend(files(&out.join("bin"), &|n| n.ends_with(".dll")));
        } else {
            wanted.extend(files(&backends, &|n| n.ends_with(".so")));
            // `libllama.so.0` and the like: the soname the linker records, which the loader looks for.
            let soname = |n: &str| {
                n.starts_with("lib")
                    && n.rsplit_once(".so.").is_some_and(|(_, v)| {
                        !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())
                    })
            };
            for lib in ["lib", "lib64"] {
                wanted.extend(files(&out.join(lib), &soname));
            }
            // Linux bundles put resources in `/usr/lib/<productName>` (`$APPDIR/usr/lib/...` in an AppImage), next to
            // `/usr/bin`. DT_RPATH (not RUNPATH) also covers the libraries' own dependencies.
            println!(
                "cargo:rustc-link-arg-bins=-Wl,--disable-new-dtags,-rpath,$ORIGIN/../lib/Cardloader/llama"
            );
        }
        assert!(
            !wanted.is_empty(),
            "no llama.cpp libraries in {}",
            out.display()
        );
    }
    // Copy what changed (`fs::copy` follows the .so symlinks) and drop what is no longer built.
    let names: Vec<_> = wanted
        .iter()
        .map(|p| p.file_name().unwrap().to_owned())
        .collect();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        if !names.contains(&entry.file_name()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    for src in &wanted {
        let dest = dir.join(src.file_name().unwrap());
        let (Ok(s), d) = (std::fs::metadata(src), std::fs::metadata(&dest)) else {
            continue;
        };
        let stale = match d {
            Ok(d) => d.len() != s.len() || s.modified().ok() > d.modified().ok(),
            Err(_) => true,
        };
        if stale {
            std::fs::copy(src, &dest).expect("copy a llama.cpp library");
        }
    }
}
