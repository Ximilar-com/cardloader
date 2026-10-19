# Cardloader — build & development tasks.  Run `just` to list them.

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

bundle_dir := "target/release/bundle"

default:
    @just --list

# Install frontend and Rust dependencies (see README for OS packages)
setup:
    npm ci
    cargo fetch

# Run the app in development mode with hot reload
dev: sidecar
    npm run tauri dev

# Build installers for this platform into target/release/bundle
build: sidecar
    npm run tauri build

# Build for a specific Rust target, e.g. `just build-target x86_64-apple-darwin`
build-target target: sidecar
    npm run tauri build -- --target {{target}}

# Build a universal macOS app (Apple silicon + Intel)
[macos]
bundle-mac-universal:
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    npm run tauri build -- --target universal-apple-darwin

# Windows only: build the 32-bit TWAIN sidecar and fetch TWAINDSM.dll (no-op elsewhere)
sidecar:
    node scripts/prepare-sidecar.mjs

# Install the built app: /Applications
[macos]
install: build
    rm -rf "/Applications/Cardloader.app"
    cp -R "{{bundle_dir}}/macos/Cardloader.app" /Applications/
    @echo "Installed /Applications/Cardloader.app"

# Install the built app: .deb via dpkg, else the AppImage into ~/.local/bin
[linux]
install: build
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v dpkg >/dev/null; then
        sudo dpkg -i {{bundle_dir}}/deb/*.deb
    else
        mkdir -p "$HOME/.local/bin"
        install -m 755 {{bundle_dir}}/appimage/*.AppImage "$HOME/.local/bin/cardloader"
        echo "Installed ~/.local/bin/cardloader"
    fi

# Install the built app: run the NSIS installer
[windows]
install: build
    $installer = Get-ChildItem "{{bundle_dir}}/nsis/*-setup.exe" | Select-Object -First 1; Start-Process -Wait $installer.FullName

# Lint and type-check everything
lint:
    npm run lint
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

# Format Rust and frontend code
fmt:
    cargo fmt --all
    npm run format

# Run all tests
test:
    npm test
    cargo test --workspace

# Release: bump the version (x.y.z, patch, minor, major), check, tag and push; CI builds a draft release
update-version version *flags:
    node scripts/release.mjs {{version}} {{flags}}

# Regenerate app icons from assets/app-icon.svg
icons:
    npx tauri icon assets/app-icon.svg -o src-tauri/icons

# Remove build output
clean:
    cargo clean
    node -e "require('fs').rmSync('dist', { recursive: true, force: true })"
