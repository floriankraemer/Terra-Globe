#!/usr/bin/env bash
# Must run on real macOS hardware or an Apple-hosted CI runner (e.g. GitHub Actions
# macos-latest) - Tauri's macOS bundle needs Xcode/the macOS SDK, which cannot be
# obtained or licensed for use in a Linux container.
set -euo pipefail

corepack enable
corepack prepare pnpm@9 --activate
pnpm install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri build
