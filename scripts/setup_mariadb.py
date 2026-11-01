"""Fetch a portable development DB only; does not initialize a DB or register a service."""

import os
from pinned_zip_tool import ensure_tool

VERSION = "11.8.9"
SHA256 = "830c46727d9278eae212ae3eca44eeb9e71b2a68704e95f344a64fba7b1963f5"
URL = "https://archive.mariadb.org/mariadb-11.8.9/winx64-packages/mariadb-11.8.9-winx64.zip"

if __name__ == "__main__":
    if os.name != "nt": raise SystemExit("This helper pins a Windows x64 development tool, not a supported production DB version.")
    try:
        target = ensure_tool(f"mariadb-{VERSION}",URL,SHA256,strip_root="mariadb-11.8.9-winx64")
        print(target/"bin/mariadbd.exe")
    except (OSError,ValueError) as error:
        raise SystemExit(f"MariaDB development tool setup failed: {error}")
