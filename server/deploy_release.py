#!/usr/bin/env python3
"""Upload one staged, signed release to the Pi after pinning its SSH host key."""
from __future__ import annotations

import argparse
import base64
import hashlib
import re
import socket
import tempfile
import uuid
from pathlib import Path, PurePosixPath

VERSION_RE = re.compile(r"^v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$")


def load_local_env() -> dict[str, str]:
    path = Path(__file__).resolve().parents[1] / ".env"
    values: dict[str, str] = {}
    if not path.is_file():
        return values
    for line in path.read_text(encoding="utf-8-sig").splitlines():
        key, separator, value = line.partition("=")
        if separator and key.strip() and not key.lstrip().startswith("#"):
            values[key.strip()] = value.strip().strip("\"'")
    return values


def remote_mkdirs(sftp, path: PurePosixPath) -> None:
    current = PurePosixPath("/") if path.is_absolute() else PurePosixPath(".")
    for part in path.parts:
        if part in ("/", "."):
            continue
        current = current / part
        try:
            sftp.stat(str(current))
        except OSError:
            sftp.mkdir(str(current))


def upload_immutable(sftp, local: Path, remote: PurePosixPath) -> None:
    try:
        remote_stat = sftp.stat(str(remote))
        if remote_stat.st_size != local.stat().st_size:
            raise SystemExit(f"Remote immutable artifact has a different size: {remote.name}")
        with tempfile.TemporaryDirectory(prefix="pr2po-release-check-") as temp:
            downloaded = Path(temp) / remote.name
            sftp.get(str(remote), str(downloaded))
            if hashlib.sha256(downloaded.read_bytes()).digest() != hashlib.sha256(local.read_bytes()).digest():
                raise SystemExit(f"Remote immutable artifact differs; use a new release version: {remote.name}")
        return
    except OSError:
        pass
    temporary = remote.with_name(f".{remote.name}.{uuid.uuid4().hex}.upload")
    try:
        sftp.put(str(local), str(temporary), confirm=True)
        sftp.rename(str(temporary), str(remote))
    finally:
        try:
            sftp.remove(str(temporary))
        except OSError:
            pass


def upload_replace(sftp, local: Path, remote: PurePosixPath) -> None:
    temporary = remote.with_name(f".{remote.name}.{uuid.uuid4().hex}.upload")
    try:
        sftp.put(str(local), str(temporary), confirm=True)
        rename = getattr(sftp, "posix_rename", sftp.rename)
        rename(str(temporary), str(remote))
    finally:
        try:
            sftp.remove(str(temporary))
        except OSError:
            pass


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--storage", type=Path, required=True, help="Staged release-storage directory")
    parser.add_argument("--version", required=True)
    parser.add_argument("--remote-dir", help="Overrides PR2PO_UPDATE_REMOTE_DIR from .env")
    args = parser.parse_args()
    if not VERSION_RE.fullmatch(args.version):
        raise SystemExit("Invalid release version")

    env = load_local_env()
    host = env.get("SSH_SERVER_HOST", "")
    port = int(env.get("SSH_SERVER_PORT", "22"))
    username = env.get("SSH_SERVER_USER", "")
    password = env.get("SSH_SERVER_PASSWORD", "")
    pin = env.get("SSH_SERVER_HOST_KEY_SHA256", "")
    remote_dir = args.remote_dir or env.get("PR2PO_UPDATE_REMOTE_DIR", "")
    if not all((host, username, password, remote_dir)):
        raise SystemExit("Set SSH_SERVER_HOST/PORT/USER/PASSWORD and PR2PO_UPDATE_REMOTE_DIR in .env")
    if not pin.startswith("SHA256:"):
        raise SystemExit("Set SSH_SERVER_HOST_KEY_SHA256 to the host fingerprint verified out of band")

    storage = args.storage.resolve()
    release = storage / "releases" / args.version
    catalog = storage / "catalog.json"
    files = storage / "files" / args.version
    if not catalog.is_file() or not (release / "manifest.json").is_file() or not files.is_dir():
        raise SystemExit("Stage this version first with publish_release.py")
    local_assets = [path for path in files.rglob("*") if path.is_file()]
    if not local_assets:
        raise SystemExit("No updater assets found for this version")

    try:
        import paramiko
    except ImportError as error:
        raise SystemExit("Install the publisher dependency with: python -m pip install -r server/requirements.txt") from error

    transport = None
    try:
        sock = socket.create_connection((host, port), timeout=15)
        transport = paramiko.Transport(sock)
        transport.start_client(timeout=15)
        key = transport.get_remote_server_key()
        fingerprint = "SHA256:" + base64.b64encode(hashlib.sha256(key.asbytes()).digest()).decode("ascii").rstrip("=")
        if fingerprint != pin:
            raise SystemExit("SSH host key fingerprint does not match the pinned value; upload stopped")
        transport.auth_password(username=username, password=password)
        if not transport.is_authenticated():
            raise SystemExit("SSH authentication failed")

        sftp = paramiko.SFTPClient.from_transport(transport)
        remote_root = PurePosixPath(remote_dir)
        remote_mkdirs(sftp, remote_root)
        for local in local_assets:
            remote = remote_root / "files" / args.version / local.relative_to(files).as_posix()
            remote_mkdirs(sftp, remote.parent)
            upload_immutable(sftp, local, remote)

        remote_release = remote_root / "releases" / args.version
        remote_mkdirs(sftp, remote_release)
        upload_replace(sftp, release / "manifest.json", remote_release / "manifest.json")
        # Publish the catalog last so clients never see a version before its files and manifest exist.
        upload_replace(sftp, catalog, remote_root / "catalog.json")
        sftp.close()
    finally:
        if transport:
            transport.close()
    print(f"Uploaded signed PR2PO {args.version} release to the configured update storage")


if __name__ == "__main__":
    main()
