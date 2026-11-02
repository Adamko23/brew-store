#!/usr/bin/env python3
"""Collect dependency metadata and available upstream license files offline."""

import json
import subprocess
import sys
from pathlib import Path


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def main():
    destination = Path(sys.argv[1])
    host = next(line.split(": ", 1)[1] for line in run("rustc", "-vV").splitlines() if line.startswith("host: "))
    metadata = json.loads(run("cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", host))
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    selected = set()

    def visit(key):
        if key in selected:
            return
        selected.add(key)
        for dependency in nodes[key]["deps"]:
            if any(kind["kind"] in (None, "build") for kind in dependency["dep_kinds"]):
                visit(dependency["pkg"])

    visit(metadata["resolve"]["root"])
    sections = [
        "Brew Store — third-party notices\n\n"
        "Brew Store's own source is MIT licensed; see LICENSE. Dependencies retain\n"
        "their own licenses. This file lists production/build dependencies and\n"
        "includes license/notice files shipped in their published source packages.\n"
        "Where a source package omits the text, its license expression and upstream\n"
        "repository are listed. Cargo.lock identifies the exact source versions.\n\n"
        "UI symbols are rendered from Apple's SF Symbols through AppKit for this\n"
        "macOS application and remain subject to Apple's applicable terms:\n"
        "https://developer.apple.com/sf-symbols/\n"
        "The Apple system font and SDK are loaded from macOS, not redistributed.\n"
    ]
    for package in sorted((packages[key] for key in selected), key=lambda p: (p["name"], p["version"])):
        if package["id"] == metadata["resolve"]["root"]:
            continue
        root = Path(package["manifest_path"]).resolve().parent
        paths = set()
        if package.get("license_file"):
            paths.add(Path(package["license_file"]))
        for entry in root.iterdir():
            if entry.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE")):
                if entry.is_dir():
                    paths.update(entry.rglob("*"))
                else:
                    paths.add(entry)
        heading = "\n" + "=" * 72 + f"\n{package['name']} {package['version']}\n"
        heading += f"License: {package.get('license') or 'see upstream license file'}\n"
        heading += f"Repository: {package.get('repository') or 'https://crates.io/crates/' + package['name']}\n"
        if package.get("authors"):
            heading += "Authors: " + ", ".join(package["authors"]) + "\n"
        sections.append(heading)
        for path in sorted(paths):
            resolved = path.resolve()
            # Third-party manifest paths must never read files outside the crate.
            if not resolved.is_relative_to(root) or not resolved.is_file():
                continue
            if resolved.stat().st_size > 2 * 1024 * 1024:
                raise RuntimeError(f"Unexpectedly large notice: {package['name']}")
            sections.append(f"\n--- {resolved.relative_to(root)} ---\n" + resolved.read_text())

    sysroot = Path(run("rustc", "--print", "sysroot"))
    for name in ("LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"):
        path = sysroot / "share/doc/rust" / name
        if path.is_file():
            sections.append(f"\n{'=' * 72}\nRust toolchain — {name}\n" + path.read_text())
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text("\n".join(sections) + "\n")
    print(f"Collected notices for {len(selected) - 1} dependency packages.")


if __name__ == "__main__":
    main()
