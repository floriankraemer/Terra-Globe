#!/usr/bin/env python3
"""Enforce the one-way dependency rule for the Rust workspace.

The TypeScript app enforced its layering by convention; here the rule is checked
in CI so a stray `use egui` in the domain crate fails the build instead of
quietly eroding the architecture.

Run from the repository root: `python3 scripts/check-rust-deps.py`
"""

from __future__ import annotations

import json
import subprocess
import sys

# Workspace-internal dependencies each crate is allowed to have.
ALLOWED_INTERNAL = {
    "terra-core": set(),
    "terra-kml": {"terra-core"},
    "terra-providers": {"terra-core"},
    "terra-storage": {"terra-core"},
    "terra-storage-sqlite": {"terra-core", "terra-storage"},
    "terra-storage-indexeddb": {"terra-core", "terra-storage"},
    "terra-storage-remote": {"terra-core", "terra-storage"},
    "terra-globe": {"terra-core", "terra-providers"},
    "terra-app": {
        "terra-core",
        "terra-globe",
        "terra-kml",
        "terra-providers",
        "terra-storage",
    },
    # The desktop binary is a composition root: it picks the provider
    # implementation and passes it to the app.
    "terra-desktop": {
        "terra-app",
        "terra-core",
        "terra-globe",
        "terra-providers",
        "terra-storage",
        "terra-storage-sqlite",
    },
    # Composition root for the browser build: picks IndexedDB and the browser
    # adapters, exactly like terra-desktop does for native.
    "terra-web": {
        "terra-app",
        "terra-providers",
        "terra-storage",
        "terra-storage-indexeddb",
    },
    "terra-testkit": {"terra-core", "terra-kml"},
    # Build/verification tool; may compose any storage adapter.
    "terra-cli": {"terra-core", "terra-kml", "terra-storage", "terra-storage-sqlite"},
}

# External crates that must never leak into the portable layers.
FORBIDDEN_EXTERNAL = {
    "terra-core": {"wgpu", "egui", "eframe", "egui-wgpu", "rusqlite", "ehttp", "tokio", "directories"},
    "terra-kml": {"wgpu", "egui", "eframe", "egui-wgpu", "rusqlite", "ehttp", "tokio"},
    "terra-providers": {"wgpu", "egui", "eframe", "egui-wgpu", "rusqlite", "tokio"},
    "terra-storage": {"wgpu", "egui", "eframe", "egui-wgpu", "rusqlite", "ehttp", "tokio"},
    "terra-globe": {"egui", "eframe", "egui-wgpu", "rusqlite", "directories", "tokio"},
}


def workspace_packages() -> dict[str, dict]:
    output = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    metadata = json.loads(output)
    return {package["name"]: package for package in metadata["packages"]}


def main() -> int:
    packages = workspace_packages()
    failures: list[str] = []

    for name, allowed in ALLOWED_INTERNAL.items():
        package = packages.get(name)
        if package is None:
            failures.append(f"{name}: listed in the checker but missing from the workspace")
            continue
        for dependency in package["dependencies"]:
            # Dev-dependencies only affect tests and never ship; test utilities
            # such as terra-testkit, proptest and pollster are allowed there.
            if dependency["kind"] is not None:
                continue
            dep_name = dependency["name"]
            if dep_name in packages:
                if dep_name not in allowed:
                    failures.append(
                        f"{name}: illegal internal dependency on {dep_name} "
                        f"(allowed: {sorted(allowed) or 'none'})"
                    )
            elif dep_name in FORBIDDEN_EXTERNAL.get(name, set()):
                failures.append(f"{name}: forbidden external dependency on {dep_name}")

    for name in packages:
        if name not in ALLOWED_INTERNAL:
            failures.append(f"{name}: crate is not covered by the dependency checker")

    if failures:
        print("dependency rule violations:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(f"dependency rule OK ({len(packages)} crates checked)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
