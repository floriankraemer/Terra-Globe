# Releasing Terra Globe (Rust)

The desktop app ships as an installable bundle for Linux, macOS and Windows, plus a browser bundle at a later stage.
Everything here runs on a machine with the platform toolchain; signing requires credentials that are not in this repository.

## What is produced

| Platform | Artifact | Tool |
|---|---|---|
| Windows | `.msi` installer + `.zip` of the binary | `cargo-dist` (WiX for the MSI) |
| Linux | `.tar.gz` archive, `.deb`, `.AppImage` | `cargo-dist` (archive), `cargo-deb`, `cargo-appimage` |
| macOS | `.app` bundle + `.dmg`, plus a `.tar.gz` archive | `cargo-bundle` + `hdiutil`, `cargo-dist` |
| All | Checksums and a generated install script | `cargo-dist` |

`dist-workspace.toml` is the cargo-dist configuration.
Run `dist plan` before the first tagged release: the config is maintained by hand and cargo-dist's schema can change between minor versions.

## Versioning

Versions live in the workspace `Cargo.toml` (`[workspace.package].version`) and are inherited by every crate.
Bump them with a single edit, and follow SemVer for the public behaviour of the data format (SQLite schema, KML round-trip).

## Steps

1. **Freeze the TypeScript app.** It is the parity reference; do not develop features in both trees.
2. **Run the full gate** on each target: `make rust-check`, plus `cargo test --workspace` with a real GPU.
3. **Validate packaging config**: `cargo dist plan` (after `cargo install cargo-dist`).
4. **Build the binaries** for each target:
   - Windows (native Windows host): `cargo build --release -p terra-desktop`.
   - Windows from Linux, no Windows toolchain: `make rust-build-windows`.
     It builds in the `rust-windows-build` container (`docker/Dockerfile.rust-windows`, cargo-xwin) and writes `dist-windows/TerraGlobe-rust-x64.exe`.
     The container builds only `terra-desktop` (no TypeScript, no Tauri), so it needs just Docker and network access for the crates and the MSVC CRT. The binary is unsigned.
     Verified output: a 23 MB PE32+ GUI executable whose imports are all Windows system DLLs (kernel32, user32, dxgi, dwrite, ole32, ...) - no MinGW or Linux runtime.
   - Linux: `cargo build --release -p terra-desktop`.
   - macOS: `cargo build --release -p terra-desktop --target aarch64-apple-darwin` and the x86_64 target.
5. **Bundle**:
   - Linux: `cargo deb -p terra-desktop` and `cargo appimage -p terra-desktop`.
   - macOS: `cargo bundle --release -p terra-desktop`, then `hdiutil create -volname "Terra Globe" -srcfolder target/release/bundle/osx -ov -format UDZO TerraGlobe.dmg`.
   - Windows: `dist build` produces the MSI; otherwise ship the `.exe` in a zip.
6. **Sign**:
   - Windows: `signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /a <binary>` with the code-signing certificate.
   - macOS: `codesign --deep --options runtime --sign "Developer ID Application: ..." Terra\ Globe.app`, then `xcrun notarytool submit ... --wait` and `xcrun stapler staple`.
7. **Tag and publish**: `git tag vX.Y.Z && git push --tags`; cargo-dist's GitHub Actions release workflow builds the archives/MSI and attaches them to the GitHub Release.

## Verification before publishing

- Fresh install on a clean VM per platform.
- Open an existing `terra-globe.db` (written by the TypeScript app) and confirm folders, placemarks, styles and overlays appear.
- Import a KML and export it again; the document must re-import identically.
- Confirm no network requests are made when the app is idle with a tile cache present (privacy pillar).

## Known gaps at the time of writing

- Browser storage works (IndexedDB + localStorage), but browser network providers (geocoding/routing) report "not available" until the async fetch pass; exports download through the browser.
- The old TypeScript application and its Playwright suite are still in the tree. Removing them is a deliberate, separate commit once the Rust beta has shipped.
