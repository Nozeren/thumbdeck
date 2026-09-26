# thumbdeck

Your projects and their commands, one click away. A desktop app (Tauri + Svelte) for
Linux and macOS.

- **Projects** (left): git repositories in `~/dev`, `~/projects` and your home folder, with
  their branch and a dot when they have uncommitted changes.
- **Toolkit** (right): buttons for the selected project, from
  [toolkit packs](https://github.com/Nozeren/thumbdeck-toolkits) (npm/pnpm/yarn scripts,
  Makefile targets, Django, Python, Docker Compose, Gradle, Cargo, Go) and your own actions.
- **Center**: the project's README, tabs like **Logs**, and the output of what you ran, with a
  tab for each at the bottom.
- **Running** (right): what you started, with status and a stop button.

Commands run with your login shell's environment, so tools from Homebrew, fnm or
`~/.local/bin` are found even when the app is started from a launcher or the Dock.

## Using it

**Projects.** Every git repository directly inside a scanned folder shows up (**+ › Add folder
to scan…** for more); **+ › Add project…** adds any folder. Pin the ones you use most (☆), hide
the others (×); hidden projects stay at the bottom, to show again (↺).

**Toolkit.** Buttons come from the packs that apply to the project (a `manage.py` brings the
Django pack, a `package.json` one button per script, ...). Press one to run it: it opens as a tab at the
bottom of the center (● running, ✓ done, ✗ failed) with its output streaming in, and when it
ends while you're in another window, you get a notification. × closes the tab; the command
keeps running, and the Running panel opens it again. Hover a button to see its command; × hides it from this project (it goes to
*hidden*, to bring back).

- **Your own actions**: **+** in the Toolkit (or `a`): a name and a command, run in the
  project folder. Tick *Ask before running* for deploys and resets, and *Run in the project's
  tmux session* for servers, watchers and shells.
- **Your own packs**: TOML files in `~/.config/thumbdeck/toolkits/`, for kinds of projects
  (or one repository) the built-in packs don't cover; the format is
  [SPEC.md](https://github.com/Nozeren/thumbdeck-toolkits/blob/main/SPEC.md).
  **+ › Update toolkit packs** gets the latest built-in packs without updating the app.

**tmux.** Enter opens the project in your terminal (kitty) in its own tmux session: Neovim in
the first window, a shell in the second, the project's venv active in both. Buttons marked ↗
(like Django's `runserver`) run in a named window of that session instead of in thumbdeck,
and keep running when thumbdeck closes; pressing one again while it runs doesn't start it
twice.

**Tabs.** The strip at the bottom of the center has the README, the project's tabs and its
runs; `1`, `2`, … switch between them. **+** there adds a tab to the project; ⚙ (or `S` in the
tab) changes its setup, and removes it. `z` (or ⤢) expands the center to the whole window.

- **Logs** shows the project's log files: JSON lines (pino, bunyan, structlog, ...) or plain
  text (Python, Django, Rust, Go, nginx, ...), with levels to show or hide, search, jumping from
  error to error, live tail, a line's full entry, and optional *sections* that fold a log into
  parts (a test, a request, a deploy step). Its setup says which folders and files to read.

**Keys.** `?` lists them wherever you are. The status line at the bottom shows who has the
keyboard (THUMBDECK, a tab, REVIEW), the project with its branch (↑ahead ↓behind, files
changed), and thumbdeck's short messages.

| key | |
| --- | --- |
| `j` / `k`, `g` / `G` | next / previous project, first / last |
| `/` | filter projects (Enter opens the first match) |
| `p` / `x` | pin / hide the project |
| Enter or `o` | open the project in tmux |
| Space, then a letter | run a Toolkit button (the letters show on the buttons) |
| `a` | add your own action |
| `s` | stop the command shown |
| `1`, `2`, … | the tabs at the bottom: README, the project's tabs, its runs |
| `]` / `[` | next / previous run |
| `z` | expand the center / back |
| Esc | close menus and forms, leave the filter |

A tab you open (or click in) takes the keyboard. Every tab follows the same rules:

| key | in every tab |
| --- | --- |
| `j` `k` `g` `G` | move |
| Enter or `l` | open, inside thumbdeck (a log, a conversation, a review) |
| `h` or Backspace | back |
| Tab / Shift+Tab | next / previous list (Agents: agents, skills, sessions; Git: changes, commits, branches) |
| `v` | review the changes side by side (Git, Pull requests) |
| `o` | open outside thumbdeck (a PR in the browser) |
| `d` / `u` | scroll the lower part (a diff, details) |
| `r` / `S` / `?` | refresh / set up the tab / its keys |
| Esc or `q` | give the keyboard back to thumbdeck |

On the review page: `n` / `N` next / previous change, `x` mark the file viewed (on to the next),
`s` side by side ↔ one column, Esc back. `-` moves to the file list: `j` / `k` move, `l` or Enter
opens the file, `h`, `-` or Esc go back to the changes.

## Install

The latest release (Linux x86_64, macOS), in seconds, no build tools:

```sh
curl -fsSL https://github.com/Nozeren/thumbdeck-releases/releases/latest/download/install.sh | bash
```

Or build it from a checkout (needs Node and Rust, see [Develop](#develop)):

```sh
./install.sh             # build this checkout and install it; again after pulling to update
./install.sh --release   # install the latest release instead
```

Either way it's installed for your user, no sudo: on Linux to `~/.local/bin` with a launcher
entry and icon (so it shows up in wofi / rofi / your app menu), on macOS to
`~/Applications/thumbdeck.app`. With [minisign](https://jedisct1.github.io/minisign/)
installed, a downloaded release's signature is checked first.

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

### Adding an extension (a tab)

Extensions are built in: one folder for the tab, one Rust module for its setup and commands.

1. `src-tauri/src/extensions/<name>/`: its `Setup` (serde, with a `Default`) and whatever it
   needs; add it to `AVAILABLE` in `src-tauri/src/extensions/mod.rs`, and its commands to
   `lib.rs`.
2. `src/lib/extensions/<name>/`: the tab and its setup form, Svelte components taking
   `TabProps` / `SetupProps` (the tab also offers `handleKey`, for when it has the keyboard,
   and `keymap`, its keys for the key bar); add them to `src/lib/extensions/index.ts`. Its keys
   go in `src/lib/keys/maps.ts`, following the rules there (`npm test` checks them).

`logs` is the example to follow.

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

