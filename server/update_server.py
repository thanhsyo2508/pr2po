#!/usr/bin/env python3
"""Small, read-only Tauri updater API and immutable artifact server."""
from __future__ import annotations

import json
import hashlib
import hmac
import os
import re
import secrets
import shutil
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

from publish_release import NAME_RE, VERSION_RE, digest, publish_release

ROOT = Path(os.environ.get("PR2PO_UPDATE_STORAGE", "./data")).resolve()
PUBLIC_URL = os.environ.get("PR2PO_UPDATE_PUBLIC_URL", "").rstrip("/")
VERSION_RE = re.compile(r"^v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$")
IDENT_RE = re.compile(r"^[A-Za-z0-9._+-]{1,40}$")
BUNDLE_RE = re.compile(r"^(?:msi|nsis|generic)$")
ADMIN_USER = os.environ.get("PR2PO_ADMIN_USER", "")
ADMIN_PASSWORD_HASH = os.environ.get("PR2PO_ADMIN_PASSWORD_HASH", "")
ADMIN_SESSION_TTL = max(300, min(int(os.environ.get("PR2PO_ADMIN_SESSION_TTL", "7200")), 86400))
MAX_UPLOAD_BYTES = max(1, min(int(os.environ.get("PR2PO_UPDATE_MAX_UPLOAD_MB", "512")), 2048)) * 1024 * 1024
REQUIRE_HTTPS = os.environ.get("PR2PO_REQUIRE_HTTPS", "true").lower() not in {"0", "false", "no"}
SESSIONS: dict[str, tuple[str, float]] = {}
LOGIN_FAILURES: dict[str, list[float]] = {}
STATE_LOCK = threading.RLock()
PUBLISH_LOCK = threading.Lock()
ADMIN_ROOT = Path(__file__).resolve().parent / "admin"


def read_json(path: Path, fallback):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return fallback


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

    def end_headers(self):
        request_path = urlsplit(self.path).path
        if not request_path.startswith(("/admin", "/api/v1/admin")):
            self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Cache-Control", "no-store" if request_path.startswith(("/api/", "/admin")) else "public, max-age=31536000, immutable")
        self.send_header("Referrer-Policy", "no-referrer")
        super().end_headers()

    def do_OPTIONS(self):
        if urlsplit(self.path).path.startswith("/api/v1/admin"):
            self.send_response(403)
            self.end_headers()
            return
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def json_response(self, status: int, payload):
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def admin_file(self, name: str):
        files = {"index.html": "text/html; charset=utf-8", "admin.css": "text/css; charset=utf-8", "admin.js": "text/javascript; charset=utf-8"}
        if name not in files:
            return self.json_response(404, {"error": "Not found"})
        path = ADMIN_ROOT / name
        try:
            body = path.read_bytes()
        except FileNotFoundError:
            return self.json_response(404, {"error": "Admin UI is not installed"})
        self.send_response(200)
        self.send_header("Content-Type", files[name])
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'")
        self.send_header("X-Frame-Options", "DENY")
        self.end_headers()
        self.wfile.write(body)

    def _secure_admin_transport(self) -> bool:
        return not REQUIRE_HTTPS or self.headers.get("X-Forwarded-Proto", "").lower() == "https"

    def _read_json(self, maximum: int = 16_384):
        length = int(self.headers.get("Content-Length", "0"))
        if length <= 0 or length > maximum:
            raise ValueError("Invalid request size")
        return json.loads(self.rfile.read(length).decode("utf-8"))

    def _bearer_user(self):
        authorization = self.headers.get("Authorization", "")
        if not authorization.startswith("Bearer "):
            return None
        token_hash = hashlib.sha256(authorization[7:].encode("utf-8")).hexdigest()
        now = time.monotonic()
        with STATE_LOCK:
            session = SESSIONS.get(token_hash)
            if not session:
                return None
            if session[1] <= now:
                SESSIONS.pop(token_hash, None)
                return None
            return session[0]

    def _require_admin(self):
        if not self._secure_admin_transport():
            self.json_response(426, {"error": "Admin session requires HTTPS"})
            return None
        user = self._bearer_user()
        if not user:
            self.json_response(401, {"error": "Admin session expired; sign in again"})
            return None
        return user

    def do_PUT(self):
        path = unquote(urlsplit(self.path).path)
        parts = path.strip("/").split("/")
        if len(parts) != 10 or parts[:4] != ["api", "v1", "admin", "artifacts"]:
            return self.json_response(404, {"error": "Not found"})
        if not self._require_admin():
            return
        _, _, _, _, version, target, arch, bundle_type, kind, filename = parts
        if (not VERSION_RE.fullmatch(version) or not NAME_RE.fullmatch(target) or not NAME_RE.fullmatch(arch)
                or not BUNDLE_RE.fullmatch(bundle_type) or kind not in {"artifact", "signature"}
                or not NAME_RE.fullmatch(filename)):
            return self.json_response(400, {"error": "Invalid release or artifact identifier"})
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            length = 0
        maximum = 32_768 if kind == "signature" else MAX_UPLOAD_BYTES
        if length <= 0 or length > maximum:
            return self.json_response(413, {"error": "Upload size is missing or exceeds the configured limit"})
        directory = ROOT / "staging" / version / target / arch / bundle_type
        directory.mkdir(parents=True, exist_ok=True)
        destination = directory / filename
        temporary = directory / f".{filename}.{secrets.token_hex(8)}.upload"
        remaining = length
        digest = hashlib.sha256()
        try:
            with temporary.open("wb") as output:
                while remaining:
                    block = self.rfile.read(min(1024 * 1024, remaining))
                    if not block:
                        raise ValueError("Upload ended before the expected length")
                    output.write(block)
                    digest.update(block)
                    remaining -= len(block)
            os.replace(temporary, destination)
        except (OSError, ValueError) as error:
            temporary.unlink(missing_ok=True)
            if isinstance(error, ValueError):
                return self.json_response(400, {"error": str(error)})
            return self.json_response(500, {"error": "Could not write the upload to server staging storage"})
        return self.json_response(201, {"name": filename, "bytes": length, "sha256": digest.hexdigest()})

    def do_GET(self):
        path = unquote(urlsplit(self.path).path)
        if path == "/admin":
            self.send_response(308)
            self.send_header("Location", "./admin/")
            self.end_headers()
            return
        if path == "/admin/":
            return self.admin_file("index.html")
        if path in {"/admin/admin.css", "/admin/admin.js"}:
            return self.admin_file(path.rsplit("/", 1)[1])
        if path == "/api/v1/admin/releases":
            if not self._require_admin():
                return
            catalog = read_json(ROOT / "catalog.json", {"releases": []})
            for release in catalog.get("releases", []):
                version = release.get("version", "")
                manifest = read_json(ROOT / "releases" / version / "manifest.json", {})
                release["platforms"] = sorted(manifest.get("platforms", {}).keys())
            return self.json_response(200, catalog)
        if path == "/api/v1/releases":
            return self.json_response(200, {"releases": read_json(ROOT / "catalog.json", {"releases": []}).get("releases", [])})
        parts = path.strip("/").split("/")
        if len(parts) == 7 and parts[:3] == ["api", "v1", "manifest"]:
            _, _, _, target, arch, bundle_type, version = parts
            if not IDENT_RE.fullmatch(target) or not IDENT_RE.fullmatch(arch) or (not BUNDLE_RE.fullmatch(bundle_type) and bundle_type != "unknown") or (not VERSION_RE.fullmatch(version) and version != "latest"):
                return self.json_response(400, {"error": "Invalid platform or release identifier"})
            catalog = read_json(ROOT / "catalog.json", {"releases": []}).get("releases", [])
            selected = next((item for item in catalog if item.get("version") == version), None) if version != "latest" else (catalog[0] if catalog else None)
            if not selected:
                self.send_response(204)
                self.end_headers()
                return
            release_version = selected.get("version", "")
            manifest = read_json(ROOT / "releases" / release_version / "manifest.json", {})
            platforms = manifest.get("platforms", {})
            platform = platforms.get(f"{target}-{arch}-{bundle_type}") or platforms.get(f"{target}-{arch}")
            if not platform:
                return self.json_response(404, {"error": "No artifact for this platform"})
            if not PUBLIC_URL:
                return self.json_response(503, {"error": "PR2PO_UPDATE_PUBLIC_URL is not configured"})
            asset = platform.get("asset", "")
            if not re.fullmatch(r"[A-Za-z0-9._+-]{1,180}", Path(asset).name):
                return self.json_response(500, {"error": "Invalid artifact name"})
            url = f"{PUBLIC_URL}/files/{release_version}/{target}/{arch}/{Path(asset).name}"
            return self.json_response(200, {"version": release_version, "notes": selected.get("notes", ""), "pub_date": selected.get("pubDate"), "url": url, "signature": platform.get("signature", "")})
        if path.startswith("/files/"):
            return super().do_GET()
        return self.json_response(404, {"error": "Not found"})

    def do_POST(self):
        path = unquote(urlsplit(self.path).path)
        if not path.startswith("/api/v1/admin/"):
            return self.json_response(404, {"error": "Not found"})
        if not self._secure_admin_transport():
            return self.json_response(426, {"error": "Admin session requires HTTPS"})
        if path == "/api/v1/admin/login":
            return self.admin_login()
        if path == "/api/v1/admin/logout":
            token = self.headers.get("Authorization", "").removeprefix("Bearer ")
            token_hash = hashlib.sha256(token.encode("utf-8")).hexdigest() if token else ""
            with STATE_LOCK:
                SESSIONS.pop(token_hash, None)
            return self.json_response(200, {"ok": True})
        if path == "/api/v1/admin/publish":
            if not self._require_admin():
                return
            return self.admin_publish()
        return self.json_response(404, {"error": "Not found"})

    def admin_login(self):
        if not ADMIN_USER or not ADMIN_PASSWORD_HASH:
            return self.json_response(503, {"error": "Admin login is not configured on the server"})
        try:
            data = self._read_json(4096)
        except (ValueError, UnicodeDecodeError, json.JSONDecodeError):
            return self.json_response(400, {"error": "Invalid login request"})
        if not isinstance(data, dict):
            return self.json_response(400, {"error": "Invalid login request"})
        username = str(data.get("username", ""))[:128]
        password = str(data.get("password", ""))[:1024]
        user_key = hashlib.sha256(username.casefold().encode("utf-8")).hexdigest()
        now = time.monotonic()
        with STATE_LOCK:
            failures = [when for when in LOGIN_FAILURES.get(user_key, []) if now - when < 600]
            if len(failures) >= 5:
                LOGIN_FAILURES[user_key] = failures
                return self.json_response(429, {"error": "Too many sign-in attempts. Try again in 10 minutes."})
        if username != ADMIN_USER or not verify_admin_password(password, ADMIN_PASSWORD_HASH):
            with STATE_LOCK:
                LOGIN_FAILURES.setdefault(user_key, []).append(now)
            time.sleep(0.2)
            return self.json_response(401, {"error": "Username or password is incorrect"})
        with STATE_LOCK:
            LOGIN_FAILURES.pop(user_key, None)
        token = secrets.token_urlsafe(32)
        token_hash = hashlib.sha256(token.encode("utf-8")).hexdigest()
        with STATE_LOCK:
            SESSIONS[token_hash] = (username, now + ADMIN_SESSION_TTL)
        return self.json_response(200, {"token": token, "expiresIn": ADMIN_SESSION_TTL, "username": username})

    def admin_publish(self):
        try:
            data = self._read_json(16_384)
            if not isinstance(data, dict):
                return self.json_response(400, {"error": "Invalid release metadata"})
            version = str(data.get("version", ""))
            target = str(data.get("target", ""))
            arch = str(data.get("arch", ""))
            bundle_type = str(data.get("bundleType", ""))
            artifact_name = str(data.get("artifactName", ""))
            signature_name = str(data.get("signatureName", ""))
            notes = str(data.get("notes", "")).strip()
            if (not VERSION_RE.fullmatch(version) or not NAME_RE.fullmatch(target) or not NAME_RE.fullmatch(arch)
                    or not BUNDLE_RE.fullmatch(bundle_type) or not NAME_RE.fullmatch(artifact_name)
                    or not NAME_RE.fullmatch(signature_name) or signature_name != f"{artifact_name}.sig"):
                return self.json_response(400, {"error": "Invalid release metadata"})
            stage = ROOT / "staging" / version / target / arch / bundle_type
            artifact = stage / artifact_name
            signature = stage / signature_name
            if not artifact.is_file() or not signature.is_file():
                return self.json_response(400, {"error": "Upload both the installer and its matching .sig file first"})
            with PUBLISH_LOCK:
                destination = publish_release(ROOT, version, notes, target, arch, bundle_type, artifact, signature)
            shutil.rmtree(stage, ignore_errors=True)
            return self.json_response(201, {"version": version, "artifact": destination.name, "sha256": digest(destination)})
        except ValueError as error:
            return self.json_response(400, {"error": str(error)})
        except OSError:
            return self.json_response(500, {"error": "Could not write the release to server storage"})

    def log_message(self, fmt, *args):
        # Avoid logging query strings or user-specific update details.
        print(f"{self.client_address[0]} - {self.command} {urlsplit(self.path).path} - {args[1] if len(args) > 1 else ''}")


def main():
    host = os.environ.get("PR2PO_UPDATE_BIND", "127.0.0.1")
    port = int(os.environ.get("PR2PO_UPDATE_PORT", "8787"))
    ROOT.mkdir(parents=True, exist_ok=True)
    ThreadingHTTPServer((host, port), Handler).serve_forever()


def verify_admin_password(password: str, encoded_hash: str) -> bool:
    try:
        algorithm, iterations, salt_text, expected_text = encoded_hash.split("$", 3)
        if algorithm != "pbkdf2_sha256":
            return False
        count = int(iterations)
        if count < 100_000 or count > 2_000_000:
            return False
        salt = bytes.fromhex(salt_text)
        expected = bytes.fromhex(expected_text)
        actual = hashlib.pbkdf2_hmac("sha256", password.encode("utf-8"), salt, count)
        return hmac.compare_digest(actual, expected)
    except (ValueError, TypeError):
        return False


if __name__ == "__main__":
    main()
