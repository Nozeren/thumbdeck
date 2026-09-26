#!/usr/bin/env python3
"""latest.json for a release: what installed thumbdecks read to find updates.

    latest_json.py VERSION NOTES_FILE DIR   (DIR has thumbdeck-<platform>.tar.gz and .sig files)
"""
import datetime, json, pathlib, sys

REPO = "https://github.com/Nozeren/thumbdeck-releases/releases/download"

version, notes, folder = sys.argv[1], pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3])
platforms = {}
for archive in sorted(folder.glob("thumbdeck-*.tar.gz")):
    platform = archive.name.removeprefix("thumbdeck-").removesuffix(".tar.gz")
    signature = archive.with_name(archive.name + ".sig").read_text().strip()
    platforms[platform] = {"url": f"{REPO}/v{version}/{archive.name}", "signature": signature}
if not platforms:
    sys.exit(f"no builds in {folder}")
print(json.dumps({
    "version": version,
    "notes": notes.read_text().strip(),
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": platforms,
}, indent=2))
