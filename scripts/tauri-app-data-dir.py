#!/usr/bin/env python3
"""Print the Tauri app data directory used by the local app config.

Set JOB_RADAR_APP_DATA_DIR to override the computed path.
"""

from __future__ import annotations

import json
import os
import platform
import sys
from pathlib import Path


def main() -> int:
    override = os.environ.get("JOB_RADAR_APP_DATA_DIR")
    if override:
        print(Path(override).expanduser())
        return 0

    repo_root = Path(__file__).resolve().parents[1]
    config_path = repo_root / "src-tauri" / "tauri.conf.json"

    with config_path.open("r", encoding="utf-8") as handle:
        config = json.load(handle)

    identifier = config.get("identifier")
    if not isinstance(identifier, str) or not identifier:
        print(f"Could not read Tauri identifier from {config_path}", file=sys.stderr)
        return 1

    home = Path.home()
    system = platform.system()

    if system == "Darwin":
        app_data_dir = home / "Library" / "Application Support" / identifier
    elif system == "Linux":
        app_data_dir = Path(os.environ.get("XDG_DATA_HOME", home / ".local" / "share")) / identifier
    elif system == "Windows":
        app_data_dir = Path(os.environ.get("APPDATA", home / "AppData" / "Roaming")) / identifier
    else:
        print(
            f"Unsupported platform {system!r}; set JOB_RADAR_APP_DATA_DIR explicitly.",
            file=sys.stderr,
        )
        return 1

    print(app_data_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
