#!/usr/bin/env python3
"""Stage one signed Tauri updater artifact and update the local release catalog."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import uuid
from datetime import datetime, timezone
from pathlib import Path

VERSION_RE = re.compile(r"^v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$")
NAME_RE = re.compile(r"^[A-Za-z0-9._+-]{1,180}$")


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def semver_key(value: str):
    core, _, suffix = value.lstrip("v").partition("-")
    return tuple(int(part) for part in core.split(".")), (0 if suffix else 1), suffix


def publish_release(storage: Path, version: str, notes: str, target: str, arch: str,
                    bundle_type: str, artifact: Path, signature: Path) -> Path:
    if not VERSION_RE.fullmatch(version):
        raise ValueError("--version must be a valid SemVer")
    if not NAME_RE.fullmatch(target) or not NAME_RE.fullmatch(arch):
        raise ValueError("Invalid target or architecture")
    if bundle_type not in {"msi", "nsis", "generic"}:
        raise ValueError("Invalid Tauri bundle type")
    if len(notes) > 5000:
        raise ValueError("Release notes are too long (maximum 5,000 characters)")
    if not artifact.is_file() or not signature.is_file():
        raise ValueError("Signed updater artifact and its .sig file are required")
    if artifact.stat().st_size <= 0 or signature.stat().st_size <= 0 or signature.stat().st_size > 32_768:
        raise ValueError("Updater artifact is empty or signature file size is invalid")
    if bundle_type == "nsis" and artifact.suffix.lower() != ".exe":
        raise ValueError("NSIS releases must use the signed installer .exe")
    if bundle_type == "msi" and artifact.suffix.lower() != ".msi":
        raise ValueError("MSI releases must use the signed installer .msi")

    storage = storage.resolve()
    asset_name = artifact.name
    if not NAME_RE.fullmatch(asset_name):
        raise ValueError("Updater artifact file name contains unsupported characters")
    signature_text = signature.read_text(encoding="utf-8").strip()
    if not signature_text:
        raise ValueError("The updater signature file is empty")

    folder = storage / "files" / version / target / arch
    destination = folder / asset_name
    source_hash = digest(artifact)
    if destination.exists() and digest(destination) != source_hash:
        raise ValueError("Release artifacts are immutable; use a new version number")
    release_dir = storage / "releases" / version
    manifest_path = release_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8")) if manifest_path.exists() else {"version": version, "platforms": {}}
    platform_key = f"{target}-{arch}" if bundle_type == "generic" else f"{target}-{arch}-{bundle_type}"
    platform = {"asset": asset_name, "sha256": source_hash, "signature": signature_text}
    prior_platform = manifest.setdefault("platforms", {}).get(platform_key)
    if prior_platform and prior_platform != platform:
        raise ValueError("A platform artifact is already published for this version; use a new version number")
    catalog_path = storage / "catalog.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8")) if catalog_path.exists() else {"releases": []}
    releases = catalog.get("releases", [])
    existing = next((item for item in releases if item.get("version") == version), None)
    if existing and existing.get("notes", "") != notes:
        raise ValueError("Release notes are immutable for a published version")

    # Complete every immutability check before writing anything. This keeps a
    # rejected publish from leaving an orphaned artifact or manifest behind.
    folder.mkdir(parents=True, exist_ok=True)
    release_dir.mkdir(parents=True, exist_ok=True)
    if not destination.exists():
        temporary_asset = destination.with_name(f".{destination.name}.{uuid.uuid4().hex}.tmp")
        try:
            shutil.copyfile(artifact, temporary_asset)
            os.replace(temporary_asset, destination)
        finally:
            temporary_asset.unlink(missing_ok=True)

    manifest["platforms"][platform_key] = platform
    manifest_tmp = manifest_path.with_suffix(".json.tmp")
    manifest_tmp.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.replace(manifest_tmp, manifest_path)

    if not existing:
        published_at = datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
        releases.append({"version": version, "notes": notes, "pubDate": published_at})
    releases.sort(key=lambda item: semver_key(item["version"]), reverse=True)
    catalog_tmp = catalog_path.with_suffix(".json.tmp")
    catalog_tmp.write_text(json.dumps({"releases": releases}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.replace(catalog_tmp, catalog_path)
    return destination


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--storage", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--notes", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--arch", required=True)
    parser.add_argument("--bundle-type", choices=("msi", "nsis", "generic"), default="generic")
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--signature", type=Path, required=True)
    args = parser.parse_args()
    try:
        destination = publish_release(args.storage, args.version, args.notes, args.target,
                                      args.arch, args.bundle_type, args.artifact, args.signature)
    except ValueError as error:
        raise SystemExit(str(error)) from error
    print(f"Staged PR2PO {args.version} for {args.target}-{args.arch}: {destination}")


if __name__ == "__main__":
    main()
