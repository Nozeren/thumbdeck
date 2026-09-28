# Plugins: the work while you're away

The plan for building [plugin-spec.md](plugin-spec.md), and its log. Read **Where it stands**
first when you're back.

## Where it stands

Not started. (Updated at the end of each step.)

## How I work

- **Branches**: `plugins` in thumbdeck and `plugins` in `src-tauri/toolkits` (the future
  thumbdeck-plugins; the official plugins, examples, SPEC and npm packages go there). One
  commit per finished, tested step (or part of a step). **Nothing pushed**, no PR, no release.
- **Each step is done** when `cargo test`, `npm test` and `svelte-check` pass, and I've seen it
  work in the dev app (`npm run tauri dev` with `XDG_CONFIG_HOME` in the scratchpad, so your
  settings stay untouched). I drive only the dev window, never send Enter or `o` on the main
  page (your real tmux), and keep screenshots few.
- **Choices the spec doesn't cover**: I pick what's closest to the spec, write it under
  **Decisions made** with the reason, and keep going. You can overturn any of them.
- **The spec follows the code**: when building shows something in the spec is wrong or
  missing, the spec changes in the same commit, and the change is logged.
- **Blocked**: if a step can't be finished, I say why under **Needs you**, and move on to what
  doesn't depend on it.
- **Not while you're away**: pushing, renaming the GitHub repo, publishing the npm packages,
  releasing, touching `~/.local/bin/thumbdeck`, your settings, or your tmux sessions.

Since nothing is pushed, the official plugins are installed from their local folders (linked)
while testing, and the first-run catalog is read from the local checkout
(`$THUMBDECK_CATALOG` points at it; the real one will be on GitHub).

## The steps

### 1. Manifests, loading, and Settings › Plugins

- `plugins.rs`: read and check `plugin.toml` (every field in the spec, plain-sentence
  errors), install from a git URL (latest tag, `#subfolder`), link a local folder, turn off,
  remove, check for updates. The `[detect]` / actions / generate code moves over from
  `packs.rs` / `actions.rs`.
- `settings.json`: the `plugins` list and each plugin's settings.
- A new **Settings** dialog with a **Plugins** section: installed plugins, their README, what
  they add, version, update available, settings form (drawn from `[[settings]]`), log, add /
  update / turn off / remove.
- The check command: `thumbdeck plugin check <folder>`.
- Done when: a manifest-only plugin can be added from a folder and from a local git repo,
  shows in Settings, turns off, updates to a newer tag, and is removed; bad manifests show
  clear errors.

### 2. The page API, and Git as the first plugin

- Plugin frames: the `plugin://` URI scheme, the injected `window.thumbdeck`, the CSS
  variables and kit, the `postMessage` bridge, frame life (kept, unloaded when idle).
- Tabs from plugins: the `+` tab menu, drawn tab setup forms, keys from the manifest checked
  against `RULES`, actions posted to the frame, `?` help and the status line from them.
- The API from the spec: context, projects, setup, settings, storage, fs, exec / run / tmux,
  ui (say, notify, confirm, openUrl, openReview, badge, mood), events, keys.
- `@thumbdeck/plugin` (types with doc comments), in the plugins repo's `packages/`.
- Port Git to `plugins/git`, then delete `extensions/git` on both sides.
- Done when: Git as a plugin does everything the built-in tab did (changes, diffs, commits,
  branches, stashes, the review page, keys).

### 3. Packs become plugins

- The backend: start on demand, JSON lines, `initialize` / `actions` / `shutdown`, restarts,
  stderr to the log; `@thumbdeck/backend` for Node.
- Each pack becomes a manifest-only plugin in `plugins/<id>/` (django, python, npm, cargo,
  go, gradle, make, compose), with `requires` where packs had it.
- Delete the old pack code: `packs.rs`, pack parts of `actions.rs`, `build.rs` bundling,
  `update_packs`, the `+ › Update toolkit packs` item, `~/.config/thumbdeck/toolkits`.
  Custom and hidden actions keep working (hidden ids stay `<plugin>:<name>`).
- The submodule stays in place for now (it *is* the plugins repo); only what the app compiles
  in from it goes.
- Done when: every project shows the same Toolkit buttons as before, from plugins.

### 4. Logs, Agents and Pull requests

- Agents and PRs as plugins (Agents sends `waiting` / `working` / avatar signals; PRs sends
  `review`).
- Logs with a Node backend; measured on a big log file against the Rust version, numbers in
  this file.
- Then the old extension system goes: `extensions/` on both sides, `AVAILABLE`, their
  commands in `lib.rs`, `avatarSignals`.
- Done when: each does what its built-in tab did.

### 5. The Plugins pane, panels and pages

- The pane below Projects: plugins with a `[view]`, their status, `Ctrl+p` / `Esc`; a view
  shown in the center.
- `[[panel]]` on the right (one order for the app, set in Settings), `[[page]]` over the whole
  window.
- The first-run screen offering the catalog.
- Reload on change for linked plugins, the per-plugin log, Inspect (devtools).
- `examples/pomodoro` (a view) and a panel example to prove them.

### 6. Docs and the plugins repo

- The plugins repo: `SPEC.md` (the spec, moved from here), `GUIDE.md` (writing a first
  plugin, step by step), `catalog.toml`, `template/`, `examples/` (hello, todos, pomodoro,
  tasks), `packages/` (plugin, backend, check), its README.
- `@thumbdeck/check` for CI.
- thumbdeck's README: plugins instead of packs and extensions; `CLAUDE.md`'s map.

## Needs you

- When you're back: rename `thumbdeck-toolkits` to `thumbdeck-plugins` on GitHub, publish the
  three npm packages, push both branches. (I won't do any of these.)
- Confirm: the avatar only reacts to Claude when the Agents plugin is installed (it sends
  `waiting` / `working`).

## Decisions made

(Each: what, why, where.)

## Log

(Each step: what was done, what was tested, commits.)
