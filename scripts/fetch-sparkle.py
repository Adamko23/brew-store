#!/usr/bin/env python3
"""Fetch the pinned upstream Sparkle distribution, preserving framework symlinks."""

import hashlib
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request

VERSION = "2.10.1"
SHA256 = "1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27"
URL = f"https://github.com/sparkle-project/Sparkle/releases/download/{VERSION}/Sparkle-{VERSION}.tar.xz"


def distribution():
    root = Path(__file__).resolve().parent.parent
    cache = root / ".cache" / "sparkle"
    destination = cache / VERSION
    if not (destination / ".verified-source").is_file():
        cache.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="fetch-", dir=cache) as temporary:
            temporary = Path(temporary)
            archive = temporary / "sparkle.tar.xz"
            print(f"Fetching Sparkle {VERSION}…", file=sys.stderr)
            with urllib.request.urlopen(URL, timeout=60) as source, archive.open("wb") as output:
                shutil.copyfileobj(source, output)
            if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
                raise SystemExit("Sparkle archive checksum mismatch; refusing to use it.")
            with tarfile.open(archive) as package:
                for member in package.getmembers():
                    path = Path(member.name)
                    if path.is_absolute() or ".." in path.parts:
                        raise SystemExit("Unexpected path in Sparkle distribution.")
            unpacked = temporary / "distribution"
            unpacked.mkdir()
            subprocess.run(["tar", "-xf", str(archive), "-C", str(unpacked)], check=True)
            if not (unpacked / "Sparkle.framework").is_dir():
                raise SystemExit("Sparkle distribution is missing its framework.")
            subprocess.run(["codesign", "--verify", "--deep", "--strict", str(unpacked / "Sparkle.framework")], check=True)
            (unpacked / ".verified-source").write_text(SHA256 + "\n")
            if destination.exists():
                raise SystemExit(f"Incomplete Sparkle cache at {destination}; inspect it before retrying.")
            unpacked.rename(destination)
    if (destination / ".verified-source").read_text().strip() != SHA256:
        raise SystemExit("Sparkle cache does not match the pinned distribution.")
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(destination / "Sparkle.framework")], check=True)
    return destination


if __name__ == "__main__":
    print(distribution())
