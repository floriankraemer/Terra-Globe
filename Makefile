.PHONY: help dev test lint build-web build-linux build-windows build-macos build-all clean

.DEFAULT_GOAL := help

help:            ## show this list of commands
	@grep -E '^[a-zA-Z_-]+:.*## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

WEB_IMAGE_DEPS = docker/Dockerfile.web package.json pnpm-workspace.yaml pnpm-lock.yaml \
	packages/core/package.json packages/map/package.json \
	packages/storage-indexeddb/package.json packages/storage-sqlite/package.json \
	packages/storage-remote/package.json packages/ui/package.json \
	e2e/playwright/package.json

TAURI_IMAGE_DEPS = docker/Dockerfile.tauri-linux package.json pnpm-workspace.yaml pnpm-lock.yaml \
	packages/core/package.json packages/map/package.json \
	packages/storage-indexeddb/package.json packages/storage-sqlite/package.json \
	packages/storage-remote/package.json packages/ui/package.json \
	e2e/playwright/package.json

.docker/web.stamp: $(WEB_IMAGE_DEPS)
	docker compose build web-dev
	@mkdir -p .docker && touch $@

.docker/tauri-linux.stamp: $(TAURI_IMAGE_DEPS)
	docker compose build tauri-linux-build
	@mkdir -p .docker && touch $@

dev: .docker/web.stamp   ## run browser dev server in a container (http://localhost:5173)
	docker compose up --no-build web-dev

test: .docker/web.stamp .docker/tauri-linux.stamp   ## run all unit tests (TS via container + Rust via cargo, if available)
	docker compose run --rm test
	@if command -v cargo >/dev/null 2>&1; then \
		cargo test --manifest-path src-tauri/Cargo.toml; \
	else \
		docker compose run --rm tauri-linux-build cargo test --manifest-path src-tauri/Cargo.toml; \
	fi

lint: .docker/web.stamp .docker/tauri-linux.stamp   ## eslint+prettier+clippy+rustfmt check
	docker compose run --rm test pnpm -r lint
	docker compose run --rm tauri-linux-build sh -c "cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check"

build-web: .docker/web.stamp   ## static browser bundle -> packages/ui/dist
	docker compose run --rm web-build

build-linux: .docker/tauri-linux.stamp   ## Tauri Linux bundle (.AppImage/.deb) via container
	docker compose run --rm tauri-linux-build cargo tauri build

build-windows: .docker/tauri-linux.stamp   ## Windows NSIS installer + raw .exe via cargo-xwin cross-compile (unsigned)
	docker compose run --rm tauri-linux-build cargo tauri build --target x86_64-pc-windows-msvc --runner cargo-xwin
	@mkdir -p dist-windows
	docker run --rm -v terra-globe_tauri_target:/target -v "$$(pwd)/dist-windows:/out" alpine sh -c "\
		cp /target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe /out/TerraGlobe-Setup-x64.exe && \
		cp /target/x86_64-pc-windows-msvc/release/terra-globe.exe /out/TerraGlobe-raw-x86_64.exe && \
		chown -R 1000:1000 /out"
	@echo "Built dist-windows/TerraGlobe-Setup-x64.exe (NSIS installer) and TerraGlobe-raw-x86_64.exe (raw binary)"
	@echo "NOTE: unsigned - Windows SmartScreen will warn on first run. Real code signing needs a cert + signtool."

build-macos:     ## Tauri macOS bundle; MUST run on real macOS (CI or hardware), not this container
	@echo "macOS builds require a macOS runner (e.g. GitHub Actions macos-latest)."
	@echo "Run: cargo tauri build   (on real macOS, see scripts/build-macos.sh)"

build-all: build-linux build-windows build-macos

clean:           ## tear down containers/volumes and local build output
	docker compose down -v
	rm -rf packages/*/dist src-tauri/target .docker
