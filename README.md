# thumbdeck

Your projects and their commands, one click away. A desktop app (Tauri + Svelte) for
Linux and macOS.

- **Projects** (left): git repositories in `~/dev`, `~/projects` and your home folder, with
  their branch and a dot when they have uncommitted changes.
- **Toolkit** (right): buttons from the [toolkit packs](https://github.com/Nozeren/thumbdeck-toolkits)
  that apply to the selected project: npm/pnpm/yarn scripts, Makefile targets, Django
  `manage.py` commands, Python (venv, pip, pytest), Docker Compose, Gradle, Cargo and Go.
  Servers and shells (like Django's `runserver`) run in a window of the project's tmux session.
  Write your own packs in `~/.config/thumbdeck/toolkits/`.
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
