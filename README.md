# thumbdeck

Your projects and their commands, one click away. A desktop app (Tauri + Svelte) for
Linux and macOS.

- **Projects** (left): git repositories in `~/dev`, `~/projects` and your home folder, with
  their branch and a dot when they have uncommitted changes.
- **Toolkit** (right): buttons from the [toolkit packs](https://github.com/Nozeren/thumbdeck-toolkits)
  that apply to the selected project: npm/pnpm/yarn scripts, Makefile targets, Django
  `manage.py` commands, Python (venv, pip, pytest), Docker Compose, Gradle, Cargo and Go.
  Servers and shells (like Django's `runserver`) run in a window of the project's tmux session.
  Write your own packs in `~/.config/thumbdeck/toolkits/`. **+ › Update toolkit packs** gets
  the latest packs (a clone in `~/.local/share/thumbdeck/toolkits`) without rebuilding.
- **Tabs** (center): next to the README, add tabs to a project with **+**. **Logs** shows the
  project's log files (JSON lines from pino, structlog, … or plain text from Python, Django, Rust,
  Go, nginx, …): levels, search, jump to errors, live tail, the full entry of a line, and optional
  sections. Set it up per project (folders, file pattern, sections) with ⚙ or `S`; `1`, `2`, …
  switch tabs, and a tab takes the keyboard until Esc (`?` in the tab lists its keys). `z` (or ⤢)
  expands the center to the whole window.
- **Running** (right): what you started, with status and a stop button; its output streams
  into the center, where the project's README is shown otherwise.

Commands run with your login shell's environment, so tools from Homebrew, fnm or
`~/.local/bin` are found even when the app is started from a launcher or the Dock.

## Install

```sh
./install.sh
```

Builds a release version and installs it for your user, no sudo: on Linux to `~/.local/bin`
with a launcher entry and icon (so it shows up in wofi / rofi / your app menu), on macOS to
`~/Applications/thumbdeck.app`. Run it again after pulling changes to update.

## Updates

thumbdeck checks for a new release on start (and every few hours); when there is one, a dot with
its version shows next to the title: click it to see what's new and update. **+ › Check for
updates** asks right away. Releases are signed; a download that isn't signed with thumbdeck's key
is refused. Builds from source (`./install.sh`, `npm run tauri dev`) update the same way once a
newer release exists; dev builds never do.

## Develop

Needs Node, Rust (`rustup default stable`) and, on Linux, WebKitGTK (`webkit2gtk-4.1`);
on macOS the Xcode command line tools (`xcode-select --install`).

```sh
git submodule update --init   # the built-in toolkit packs
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # build the app
```

The toolkit packs are compiled in from the `src-tauri/toolkits` submodule. To try changes to
packs in another checkout, point `THUMBDECK_TOOLKITS` at it:
`THUMBDECK_TOOLKITS=~/dev/thumbdeck-toolkits npm run tauri dev`.

## Releasing

```sh
scripts/release.sh 0.2.0          # sets the version, commits, tags v0.2.0
git push origin main v0.2.0       # GitHub Actions builds, signs and publishes it
```

The workflow (`.github/workflows/release.yml`) builds Linux x86_64 and macOS (universal),
signs the builds with the updater key (`~/.tauri/thumbdeck.key`; the public half is
`src-tauri/src/updater.pub`) and publishes them, with the `latest.json` apps read, to the public
[thumbdeck-releases](https://github.com/Nozeren/thumbdeck-releases) repo. To try an update
before publishing it, point a release build at a local copy:
`THUMBDECK_UPDATE_URL=file:///path/to/latest.json thumbdeck`.

