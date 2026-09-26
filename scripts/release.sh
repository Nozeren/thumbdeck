#!/usr/bin/env bash
# Make a release: set the version everywhere, commit, and tag. Pushing the tag builds and
# publishes it (.github/workflows/release.yml).
#   scripts/release.sh 0.2.0
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

version="${1:-}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "usage: scripts/release.sh X.Y.Z" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "commit or stash your changes first" >&2; exit 1; }
[ "$(git branch --show-current)" = main ] || { echo "releases are made from main" >&2; exit 1; }
git rev-parse -q --verify "refs/tags/v$version" >/dev/null && { echo "v$version exists already" >&2; exit 1; }

node -e '
  const fs = require("fs"), v = process.argv[1];
  for (const f of ["package.json", "src-tauri/tauri.conf.json"]) {
    const j = JSON.parse(fs.readFileSync(f, "utf8")); j.version = v;
    fs.writeFileSync(f, JSON.stringify(j, null, 2) + "\n");
  }' "$version"
npm install --package-lock-only --ignore-scripts --no-audit --no-fund >/dev/null
sed -i.bak "0,/^version = \".*\"/s//version = \"$version\"/" src-tauri/Cargo.toml && rm src-tauri/Cargo.toml.bak
(cd src-tauri && cargo update -p thumbdeck --offline -q 2>/dev/null || cargo metadata -q >/dev/null)

git commit -qam "Release $version"
git tag -a "v$version" -m "thumbdeck $version"
echo "Tagged v$version. Publish it with:  git push origin main v$version"
