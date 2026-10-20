# thumbdeck

**Your projects and their commands, one key away.** A small desktop app for Linux and macOS
(Tauri + Svelte) that sits next to your terminal: pick a project, run its commands, read its
logs, see its git changes and pull requests, and jump into its tmux session when it's time to
write code.

![thumbdeck: projects on the left, the selected project's README in the center, Running and Toolkit on the right](docs/screenshot.png)

- **Projects** on the left: the git repositories in `~/dev`, `~/projects` and your home folder,
  with their branch and a dot when they have uncommitted changes.
- **The center**: the project's Overview, its README, its pages from plugins (Git, Logs, Agents,
  …) and the output of what you ran. Its header says which: `webshop ⎇ main › Git`.
  The **Overview** shows where the project stands, in cards: how each Toolkit action ran last,
  and cards from plugins (Git's: the uncommitted files and the last commits; Notes': the open
  checklist items). A project opens
  on the page you showed last in it.
- **Running and Toolkit** on the right: what's running now (one line when nothing is; ▸ shows
  the finished runs), and the project's buttons: its commands, then its **Pages** (Overview,
  README, its plugin pages A–Z, and the ones you could add).
- **Plugins** below the projects, A–Z: plugins with a view of their own (`Ctrl+p`), like Pull
  requests or Calendar. They're about everything, not one project: opening one hides Running
  and the Toolkit until you go back to a project.
- The pane with the keyboard has a border in its own colour: Projects purple, Plugins aqua,
  Running blue, Toolkit orange, the center green.
- **The status line** at the bottom: who has the keyboard, the project's branch, and short
  messages.

Everything works from the keyboard, and `?` always lists the keys for where you are. Almost
everything thumbdeck shows comes from [plugins](#plugins).

## Projects and the Toolkit

Every git repository directly inside a scanned folder shows up (**+ › Add folder to scan…** for
more), and **+ › Add project…** adds any folder. Pin the ones you use most (☆), hide the
others (×); hidden projects stay at the bottom, to show again (↺).

The Toolkit's buttons come from plugins that recognise the project: npm / pnpm / yarn scripts,
Makefile targets, Django, Python, Docker Compose, Gradle, Cargo, Go. Press one, or Space then its letter, and it runs: it shows
in Running and its output streams into the center (● running, ✓ done, ✗ failed). Pick a run in
Running (or `[` / `]`) to show its output again. When it ends while you're in another window,
you get a notification.

- **Your own actions**: **+** in the Toolkit (or `a`): a name and a command, run in the project
  folder. Tick *Ask before running* for deploys and resets, and *Run in the project's tmux
  session* for servers, watchers and shells.
- **Buttons for a kind of project** thumbdeck doesn't know: a plugin can be just a
  `plugin.toml` saying when it applies and what the buttons run
  ([the guide](https://github.com/Nozeren/thumbdeck-plugins/blob/main/GUIDE.md)).
- Hover a button to see its command; × hides it from this project.

Commands run with your login shell's environment, so tools from Homebrew, fnm or `~/.local/bin`
are found even when thumbdeck is started from a launcher or the Dock.

## tmux

Enter opens the project in your terminal (kitty) in its own tmux session: Neovim in the first
window, a shell in the second, the project's Python venv active in both. Buttons marked ↗ (like
Django's `runserver`) run in a named window of that session instead, and keep running when
thumbdeck closes; pressing one again while it runs doesn't start it twice.

## Plugins

thumbdeck's tabs, Toolkit buttons, panels and views come from plugins, installed in **Settings**
(`,`, or the **⚙** in the Plugins title) **› Plugins**: from the [official ones](https://github.com/Nozeren/thumbdeck-plugins),
from any git URL, or from a folder you're working on. The first start offers the official ones.
thumbdeck looks for newer releases of them when it starts, and marks them (↑) in Settings,
where they're listed by name and you update them (one by one, or all at once with **Update all**), turn them off, set the order of their panels,
or read their log. A plugin runs with your
rights, like an editor plugin: install the ones you trust.

Writing one: the [guide](https://github.com/Nozeren/thumbdeck-plugins/blob/main/GUIDE.md) and
the [spec](https://github.com/Nozeren/thumbdeck-plugins/blob/main/SPEC.md). A plugin can be
just a `plugin.toml` with Toolkit buttons, or add HTML pages (tabs, panels, full-window pages,
a view) that use `window.thumbdeck`, and a backend in any language.

## Pages

The Toolkit's **Pages** are what the center can show for the project: Overview, README, then the
plugin pages the project has, A–Z, each with its plugin's icon. Dashed **+** pages are plugins
that would work here but aren't added: a click opens their setup and adds the page. `1`, `2`, …
pick a page (the small numbers), Space and a letter does too, Esc in the center goes back to the
Overview, and `z` gives the center the whole window. `S` in a page changes its setup or removes
it. The official plugins' pages:

**Logs**: the project's log files, JSON lines (pino, bunyan, structlog, …) or plain text
(Python, Django, Rust, Go, nginx, …). Show or hide levels, search, jump from error to error,
follow a live tail, open a line's full entry, and fold a log into *sections* (a test, a request,
a deploy step). Its setup says which folders and files to read.

**Agents**: the Claude Code subagents and skills of the project (and yours), each with its
description, model and tools. Pick one, say what it should do, and Enter starts Claude on it in
the project's tmux. The Sessions list shows past Claude Code sessions with their cost, and each
session's conversation, including what its subagents did step by step.

**Git**: the repo at a glance. Uncommitted changes with their diffs, the branch against its
upstream, recent commits, branches and stashes. It only looks; it never changes the repo.

**Env**: the project's `.env` against its `.env.example`: what's missing, empty or still the
example's placeholder. Values stay hidden unless you ask; `a` adds the missing keys.

**Notes**: a scratchpad and checklist for each project, kept by thumbdeck (not in the repo).

Plugins with a **view** of their own are listed below the projects, A–Z: `Ctrl+p` (or `Ctrl+j`), then
Enter, shows one in the center. **Panels** from plugins sit under the Toolkit. The official views:

- **Pull requests**: the open PRs that concern you in every repo you're in, in the order to deal
  with them: asked to review again, asked to review, reviewed, yours, each with its repo. The
  highlighted one's checks, conflicts, labels, comments and reviewers show below; 🔔 marks unread
  notifications. `v` reviews it ([the review page](#the-review-page)), `o` opens it in the
  browser. It uses your `gh` login; its Repos setting limits it to some owners or repos, and
  `demo` tries it with sample PRs.
- **Calendar**: your calendars' coming events (iCal links) or the whole month (`m`), what's next
  in the pane, reminders.
- **Azure Boards**: your Azure DevOps work items with their details; `y` copies a branch name
  for one. It needs a personal access token (Work Items: Read), kept in a Settings field that
  hides it; set the organization to `demo` to try it.
- **Chrome**: your browser's profiles grouped by tenant, one key to open each and its
  shortcuts; make, edit and delete profiles; link rules open thumbdeck's links in the profile
  signed in there.
- **Ports**: what's listening on which port and the project it runs in; `x` stops it (it asks).
- **Clipboard**: what you copied, newest first and searchable; `y` copies one again, `p` pins it.
  Password managers' copies aren't kept.
- **Dev utils**: paste a JWT, JSON, base64, a timestamp or a URL and see it decoded; UUIDs.

## The review page

`v` on a pull request, or Enter on the Git tab's changes, a commit or a stash, opens its changes
over the whole window: the files on the left (a tree, with a viewed bar and a filter, `/`), the
diff in the middle with a line cursor, and for a PR its description, checks, reviewers and labels
on the right (`i`). Every button shows its key. Nothing is checked out.

- **Reading**: `j` / `k` move, `n` / `N` jump between changes, `]` / `[` between files, `s` side
  by side or one column, with syntax colours and the words that changed marked. Enter on
  `⋯ 12 unchanged lines` shows them, `f` the whole file. `x` ticks a file viewed and moves on to
  the next (remembered until that file changes again). In the file list (`-`), each file shows as
  you move to it. The thin bar on the right shows where the changes and comments are.
- **Commenting** (a PR): `c` comments on the line, `V` selects lines (`j` / `k` or Shift+click
  extend) to comment on together, `Ctrl+g` writes a suggested change, `Ctrl+Enter` adds it to your
  review. The threads show under their lines: `t` / `T` jump between them, Enter replies, `z`
  resolves (or shows a resolved one).
- **Finishing** (a PR): `A` approve, `R` request changes, `P` post your review; it goes to GitHub
  with your line comments. Comments you haven't posted are kept if you close the page.

The Git tab's reviews are read-only (there's no PR to comment on).

## The avatar

A small pixel character sits on the bottom edge of the Toolkit and shows what's going on: it
cheers when a run finishes, sighs when one fails, waves when Claude is waiting for you, thinks
while Claude works (with the Agents plugin), holds up a sign when PRs wait for your review (Pull
requests), and sleeps when you're away. When its mood changes, a bubble says why for a few
seconds (hover it to see it again), and a soft glow takes the mood's colour. Click it to go to the
cause: the run's output, Pull requests, the Agents page. Click the title to pick another
character: octopus, crab, keycap, ghost, axolotl, or none.

## Keys

`?` lists them wherever you are. The main page is panes, like nvim splits or tmux panes:
Projects and Plugins on the left, the center, Running and Toolkit on the right. The one with
the keyboard has a bright green border and a green title, and its name is at the start of the
bottom line.

| key | everywhere |
| --- | --- |
| `Ctrl+h` / `Ctrl+j` / `Ctrl+k` / `Ctrl+l` | the pane to the left / below / above / right (from a plugin's tab too) |
| Space, then a letter | run a Toolkit button or show a page (the letters show on them) |
| `1`, `2`, … | a page: Overview, README, the project's pages A–Z (their numbers show in the Toolkit) |
| `Ctrl+b`, then `n` / `p` | the next / previous page, as in tmux (the keyboard stays where it is) |
| `]` / `[` | next / previous run |
| `a` | add your own action |
| `s` | stop the command shown |
| `z` | expand the center / back |
| `,` | settings: plugins |
| `Ctrl+p` | the Plugins pane |
| Esc | close menus and forms, leave the filter; then back to Projects |

In each pane, `j` / `k` move (`g` / `G`: first / last) and `q` goes back to Projects:

| pane | keys |
| --- | --- |
| Projects | `l` the center · Enter or `o` open the project in tmux · `/` filter, in the Projects title (Enter opens the first match) · `p` / `x` pin / hide |
| Plugins | `l` or Enter shows a plugin's view |
| Center: the Overview | `h` / `j` / `k` / `l` between the cards · Enter opens one (Last runs: the Toolkit; Changes: the review; Recent commits: the Git tab; Notes: the Notes tab) · `x` hides a card in this project (bring it back from "hidden cards" below them) · the card's own keys (`?` lists them) |
| Center: the README, a run's output | `j` / `k` scroll · `d` / `u` half a page · Esc back to the Overview |
| Running | `l` or Enter shows its output · `s` stops it |
| Toolkit | Enter runs the button or shows the page · `e` edits your own action |

A tab you open (or click in) takes the keyboard, and every tab follows the same rules (plugins
that don't aren't loaded):

| key | in every page |
| --- | --- |
| `j` `k` `g` `G` | move |
| Enter or `l` | open, inside thumbdeck (a log, a conversation, a review) |
| `h` or Backspace | back |
| Tab / Shift+Tab | next / previous list |
| `v` | review the changes |
| `o` | open outside thumbdeck (a PR in the browser) |
| `d` / `u` | scroll the lower part (a diff, details) |
| `r` / `S` / `?` | refresh / set up the tab / its keys |
| Esc or `q` | give the keyboard back to thumbdeck |

## Install

The latest release (Linux x86_64, macOS), in seconds, no build tools needed:

```sh
curl -fsSL https://github.com/Nozeren/thumbdeck-releases/releases/latest/download/install.sh | bash
```

Or from a checkout (needs Node and Rust, see [Develop](#develop)):

```sh
./install.sh             # build this checkout and install it; again after pulling to update
./install.sh --release   # install the latest release instead
```

It installs for your user, without sudo: on Linux to `~/.local/bin`, with a launcher entry and
icon (so it shows up in wofi, rofi or your app menu); on macOS to `~/Applications/thumbdeck.app`.
With [minisign](https://jedisct1.github.io/minisign/) installed, a downloaded release's
signature is checked first.

## Updates

thumbdeck looks for a new release when it starts, and every few hours. When there is one, its
version shows next to the title: click it to see what's new and update. The **↻** button
next to the title asks right away. Releases are signed, and a download that isn't signed with
thumbdeck's key is refused. Builds from source update the same way once a newer release
exists; dev builds never do.

## Develop

Needs Node, Rust (`rustup default stable`) and, on Linux, WebKitGTK (`webkit2gtk-4.1`); on
macOS, the Xcode command line tools (`xcode-select --install`).

```sh
git submodule update --init   # the plugins repo (src-tauri/toolkits), for the tests
npm install
npm run tauri dev             # run with hot reload
npm run tauri build           # build the app
```

Checks: `cd src-tauri && cargo test`, `npm test`, and
`npx svelte-check --tsconfig ./tsconfig.json`.

The plugins repo is checked out in `src-tauri/toolkits` (it isn't compiled in: its tests check
that the official plugins load, and that thumbdeck and `@thumbdeck/check` agree on the keys).
To work on a plugin, link its folder in Settings › Plugins; `THUMBDECK_CATALOG=<file or URL>`
replaces the catalog the first start offers.

### Adding a tab

Tabs are plugins: see the [guide](https://github.com/Nozeren/thumbdeck-plugins/blob/main/GUIDE.md).
thumbdeck's side of plugins is `src-tauri/src/plugins` (manifests, installing, frames, the page
API, backends) and `src/lib/plugins` (frames, setup forms).

### Releasing

```sh
scripts/release.sh 0.4.0          # sets the version, commits, tags v0.4.0
git push origin main v0.4.0       # GitHub Actions builds, signs and publishes it
```

The workflow (`.github/workflows/release.yml`) builds Linux x86_64 and macOS (universal), signs
the builds with the updater key (`~/.tauri/thumbdeck.key`; the public half is
`src-tauri/src/updater.pub`), and publishes them with the `latest.json` apps read to the public
[thumbdeck-releases](https://github.com/Nozeren/thumbdeck-releases) repo. To try an update
before publishing it, point a release build at a local copy:
`THUMBDECK_UPDATE_URL=file:///path/to/latest.json thumbdeck`.
