#!/usr/bin/env python3
"""Sign release metadata with Sparkle; the private key stays in macOS Keychain."""

import base64
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
ACCOUNT = "io.github.adamko23.brew-store"
SPARKLE = "http://www.andymatuschak.org/xml-namespaces/sparkle"


def signing_tools():
    path = subprocess.check_output(
        [sys.executable, str(ROOT / "scripts/fetch-sparkle.py")], text=True
    ).strip()
    tools = Path(path) / "bin"
    public = subprocess.check_output(
        [str(tools / "generate_keys"), "--account", ACCOUNT, "-p"], text=True, timeout=45
    ).strip()
    if public != (ROOT / "assets/update-public-key.txt").read_text().strip():
        raise SystemExit("The Keychain signing key does not match the public key embedded in Brew Store.")
    if len(base64.b64decode(public, validate=True)) != 32:
        raise SystemExit("Invalid update signing public key.")
    return tools


def generate(archive):
    tools = signing_tools()
    archive = archive.resolve(strict=True)
    version = archive.name.removeprefix("Brew-Store-").removesuffix("-macos-arm64.zip")
    if archive.name != f"Brew-Store-{version}-macos-arm64.zip" or not all(
        part.isdecimal() for part in version.split(".")
    ):
        raise SystemExit("Expected a versioned Apple Silicon release ZIP.")
    prefix = f"https://github.com/Adamko23/brew-store/releases/download/v{version}/"
    with tempfile.TemporaryDirectory(prefix="brew-store-appcast-") as temporary:
        stage = Path(temporary)
        shutil.copy2(archive, stage / archive.name)
        previous = ROOT / "appcast.xml"
        if previous.exists():
            shutil.copy2(previous, stage / "appcast.xml")
        notes = ROOT / "docs/releases" / f"v{version}.md"
        if notes.exists():
            shutil.copy2(notes, stage / archive.with_suffix(".md").name)
        subprocess.run([
            str(tools / "generate_appcast"), "--account", ACCOUNT,
            "--download-url-prefix", prefix, "--embed-release-notes",
            "--maximum-deltas", "0", "--link", "https://github.com/Adamko23/brew-store",
            str(stage),
        ], check=True)
        feed = stage / "appcast.xml"
        subprocess.run([
            str(tools / "sign_update"), "--account", ACCOUNT, "--verify", str(feed),
        ], check=True)
        xml = ET.parse(feed)
        item = next((item for item in xml.findall("./channel/item")
                     if item.findtext(f"{{{SPARKLE}}}version") == version), None)
        if item is None:
            raise SystemExit("Generated appcast is missing the new version.")
        enclosure = item.find("enclosure")
        if enclosure is None or enclosure.get("url") != prefix + archive.name:
            raise SystemExit("Appcast points to an unexpected release archive.")
        if int(enclosure.get("length", "0")) != archive.stat().st_size:
            raise SystemExit("Appcast archive size does not match.")
        signature = enclosure.get(f"{{{SPARKLE}}}edSignature", "")
        if len(base64.b64decode(signature, validate=True)) != 64:
            raise SystemExit("Appcast does not contain a valid EdDSA signature.")
        subprocess.run([
            str(tools / "sign_update"), "--account", ACCOUNT, "--verify", str(archive), signature,
        ], check=True)
        if item.findtext(f"{{{SPARKLE}}}hardwareRequirements") != "arm64":
            raise SystemExit("Appcast must restrict this build to Apple Silicon.")
        if item.findtext(f"{{{SPARKLE}}}minimumSystemVersion") not in {"13.0", "13.0.0"}:
            raise SystemExit("Unexpected minimum macOS version in appcast.")
        output = archive.parent / "appcast.xml"
        shutil.copy2(feed, output)
        print(f"Signed and verified: {output}")


if __name__ == "__main__":
    if sys.argv[1:] == ["--check-key"]:
        signing_tools()
        print("Keychain signing key matches Brew Store's public update key.")
    elif len(sys.argv) == 2:
        generate(Path(sys.argv[1]))
    else:
        raise SystemExit("Usage: generate-appcast.py --check-key | RELEASE.zip")
