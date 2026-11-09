#!/usr/bin/env python3
"""Create a PBKDF2 password hash for PR2PO_UPDATE's admin environment."""
from __future__ import annotations

import argparse
import getpass
import hashlib
import secrets


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--iterations", type=int, default=600_000)
    args = parser.parse_args()
    if not 300_000 <= args.iterations <= 2_000_000:
        raise SystemExit("Iterations must be between 300,000 and 2,000,000")
    first = getpass.getpass("Admin password (minimum 12 characters): ")
    second = getpass.getpass("Confirm password: ")
    if len(first) < 12:
        raise SystemExit("Password must contain at least 12 characters")
    if first != second:
        raise SystemExit("Passwords do not match")
    salt = secrets.token_bytes(16)
    derived = hashlib.pbkdf2_hmac("sha256", first.encode("utf-8"), salt, args.iterations)
    print("PR2PO_ADMIN_PASSWORD_HASH=pbkdf2_sha256${}${}${}".format(
        args.iterations, salt.hex(), derived.hex()
    ))


if __name__ == "__main__":
    main()
