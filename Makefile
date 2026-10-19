# Cardloader — the same tasks as the Justfile, for those without `just` (https://just.systems).
# On Windows prefer `just` (PowerShell recipes).

.PHONY: help setup dev build sidecar lint fmt test icons clean install update-version

BUNDLE_DIR := target/release/bundle
UNAME := $(shell uname -s)

help:
	@echo "Targets: setup dev build install lint fmt test icons clean update-version"

setup:
	npm ci
	cargo fetch

sidecar:
	node scripts/prepare-sidecar.mjs

dev: sidecar
	npm run tauri dev

build: sidecar
	npm run tauri build

install: build
ifeq ($(UNAME),Darwin)
	rm -rf "/Applications/Cardloader.app"
	cp -R "$(BUNDLE_DIR)/macos/Cardloader.app" /Applications/
else
	@if command -v dpkg >/dev/null; then \
		sudo dpkg -i $(BUNDLE_DIR)/deb/*.deb; \
	else \
		mkdir -p "$$HOME/.local/bin" && install -m 755 $(BUNDLE_DIR)/appimage/*.AppImage "$$HOME/.local/bin/cardloader"; \
	fi
endif

lint:
	npm run lint
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all
	npm run format

test:
	npm test
	cargo test --workspace

# make update-version VERSION=0.2.0 (or patch/minor/major) [FLAGS="--dry-run"]
update-version:
	@test -n "$(VERSION)" || { echo "usage: make update-version VERSION=0.2.0 [FLAGS=--dry-run]"; exit 1; }
	node scripts/release.mjs $(VERSION) $(FLAGS)

icons:
	npx tauri icon assets/app-icon.svg -o src-tauri/icons

clean:
	cargo clean
	rm -rf dist
