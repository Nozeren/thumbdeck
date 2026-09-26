# thumbdeck

Your projects and their commands, one click away. A desktop app (Tauri + Svelte) for
Linux and macOS.

- **Projects** (left): git repositories in `~/dev`, `~/projects` and your home folder, with
  their branch and a dot when they have uncommitted changes.
- **Toolkit** (right): buttons detected from the selected project's files: npm/pnpm/yarn
  scripts, Makefile targets, Django `manage.py` commands, pytest, Docker Compose, Gradle,
  Cargo and Go.
- **Running** (right): what you started, with status and a stop button; its output streams
  into the center, where the project's README is shown otherwise.

Commands run with your login shell's environment, so tools from Homebrew, fnm or
`~/.local/bin` are found even when the app is started from a launcher or the Dock.

## Develop

Needs Node, Rust (`rustup default stable`) and, on Linux, WebKitGTK (`webkit2gtk-4.1`);
on macOS the Xcode command line tools (`xcode-select --install`).

```sh
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # build the app
```
