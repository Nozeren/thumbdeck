# Plugins: the work while you're away

The plan for building [plugin-spec.md](plugin-spec.md), and its log. Read **Where it stands**
first when you're back.

## Where it stands

Steps 1 to 5 are done: plugins are installed and managed in Settings (`,`), the first start
offers the official ones, plugin tabs, views (the Plugins pane, `Ctrl+p`), panels and pages
run in frames with the whole page API, plugins can have a backend, and every tab thumbdeck
had and every pack is a plugin now; the built-in extensions and the pack code are gone.
Step 6 (docs and the plugins repo) is next.

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
  three npm packages, push both branches: the submodule's `plugins` branch first, since
  thumbdeck's commits point at its commits. (I won't do any of these.)
- Confirm: the avatar only reacts to Claude when the Agents plugin is installed (it sends
  `waiting` / `working`).

## Decisions made

1. **Settings opens with `,`** (and **+ › Settings…**): there was no settings dialog yet, and
   `,` is free and means settings in many apps. `maps.ts`.
2. **Plugins live in `$XDG_DATA_HOME/thumbdeck/plugins`** (default `~/.local/share/thumbdeck`),
   so a dev run can keep them apart from yours, like `XDG_CONFIG_HOME` does for settings.
   `plugins/install.rs`.
3. **Each installed plugin is a clone of its own**, even when several come from one
   repository, so each can sit at its own release tag. `plugins/install.rs`.
4. **A plugin with any problem isn't loaded**; there are no warnings. Simple to understand, and
   `plugin check` says exactly the same. `plugins/manifest.rs`.
5. **Until step 3, packs and plugins both fill the Toolkit**; a plugin replaces the pack with
   its id, so a pack can be tried as a plugin next to the others. `lib.rs` `providers`.
6. **`[detect]`'s `icon`** is one of thumbdeck's icon names or an `.svg` in the plugin (the
   .svg isn't drawn yet: step 3).
7. **TOML key typos are said plainly**: "there's no key nmae (did you mean name?)" instead of
   serde's list of every key.
8. **The key rules are in Rust too** (`plugins/keys.rs`), for checking manifests; a test fails
   if they differ from `RULES` in `keys.ts`.
9. **A plugin that adds nothing is still valid** (e.g. while you start writing one).
10. **A frame's context travels in its address** (`plugin://git/tab.html?td=<base64 JSON>`) and
    the API script is put inline at the top of the page, so `thumbdeck.context` is there at
    once, with no request. `plugins/frame.rs`, `src/lib/plugins/frame.ts`.
11. **Frames don't get thumbdeck's own IPC**: everything goes through `postMessage` to the
    page, which attaches the plugin and project itself, so a frame can't pretend to be another
    plugin. `PluginFrame.svelte`.
12. **`td.run` adds its output tab without switching to it**, so the plugin's page stays in
    front (a Toolkit button still switches).
13. **`fs.watch` looks once a second** (a stat) instead of using the system's file watcher:
    simple, and enough for logs and config files.
14. **The plugin log came early** (planned for step 5): I needed it to debug the first frame.
    Pages' console and uncaught errors, in Settings › Plugins › Log, and in the terminal in
    dev builds.
15. **The status line's branch check stays in thumbdeck** (`branch.rs`, `branch_status`); the
    rest of the old Git module went with the built-in Git tab.
16. **A review's "viewed" marks are kept per plugin** (`plugin:git:…`), so plugins can't mix up
    each other's; the marks from the built-in Git tab aren't carried over.
17. **Old built-in tabs that are plugins now** show a short note with a Remove button, instead
    of disappearing silently.
18. **The official plugins are tested by thumbdeck's Rust tests** (`plugins/official.rs`, reading
    the submodule): the tests that checked the bundled packs now check that the plugins give
    the same buttons and icons.
19. **Your own packs folder (`~/.config/thumbdeck/toolkits`) isn't read any more**: a pack
    becomes a plugin by adding `id`, `version` and `api` (and moving `icon`, `priority`,
    `requires` under `[detect]`); link its folder in Settings.
20. **A backend's Toolkit buttons are asked for in the background**: the Toolkit shows at once
    and they join when the backend answers (a first start reads your shell's environment and
    starts node, a few seconds).
21. **`[backend] install` runs for installed plugins, not linked folders** (a folder you're
    working in is yours to `npm install`).
22. **A command source without `watch` is kept for a minute.**
23. **A backend's `ui.badge` goes on all the plugin's tabs** (a backend isn't tied to one).
24. **Pull requests is plain JS; Agents (and Logs) are Svelte, built with Vite** into the
    plugin's `dist/` (committed, so installing needs no build). Their UI code stayed close to
    the old Svelte tabs, which was safer than rewriting them; the official plugins now show
    both ways of writing one.
25. **`[backend] autostart`** (new): the Agents backend starts with thumbdeck, so the avatar
    knows about Claude Code sessions without the Agents tab open.
26. **`td.tmux(…, { show: true })`** (new): starting an agent types the command into the
    `claude` window and shows it, as the built-in tab did.
27. **The avatar's Claude signals come from the Agents plugin**: without it, the avatar
    doesn't react to Claude (the question under Needs you).
28. **`.taurignore` leaves out the plugins repo**: editing a plugin no longer restarts the dev
    app. (A frame in the unfocused dev window sometimes didn't repaint until a key was pressed;
    the page itself was right. Worth watching; I couldn't reproduce it with the window in front.)
29. **Logs in Node is faster than it was in Rust**: a 200,000-line JSON log (35 MB) reads in
    0.7–0.8 s in Node against 1.3 s in Rust (the JSON sent to the page: about 130 ms either way).
30. **`setup_page` and a `json` field type** (both new in the spec for Logs): its sections
    can't be a drawn form. The page shows in thumbdeck's setup dialog and gets the focus.
31. **Plugin tabs are saved as `tabs: { plugin, tab, setup }`** in settings.json; the old
    `extensions` list is dropped when thumbdeck saves (as decided: old tabs start fresh).
    Removing a plugin removes its tabs.
32. **The first start offers the whole catalog, every plugin ticked**: all of it used to be
    built in, so an update keeps what you had with one Enter. It's asked once
    (`catalog_offered`); offline, it's quietly asked again next start. Settings › Plugins ›
    Add a plugin lists the ones not installed.
33. **The catalog comes with `curl`** (like the updater), from the plugins repo on GitHub;
    `$THUMBDECK_CATALOG` (a URL or a file) takes its place, for trying it before it's pushed.
34. **A project panel loads again when you pick another project** (app panels and views stay);
    keeping one per project didn't seem worth the memory for a glance.
35. **Inspect opens the inspector for the whole window** (Tauri's `devtools` feature, so the
    installed app has it too); the plugin's frame is picked in it.
36. **Panels follow the plugins' order**, changed with ↑ ↓ in Settings › Plugins.
37. **A linked plugin reloads when any of its files changes** (looked at every 1.2 s; its
    `node_modules` and `.git` aren't): its frames load again, its backend restarts.

## Log

### Step 1: manifests, installing, Settings › Plugins

- `src-tauri/src/plugins/`: `manifest.rs` (plugin.toml and every check), `install.rs` (git
  URL with `#folder`, latest `v1.2.0` / `<id>-v1.2.0` tag, updates, links, removing),
  `detect.rs` and `toolkit.rs` (moved from `packs.rs` / `actions.rs`, now shared by packs and
  plugins), `keys.rs` (the rules), `mod.rs` (loading, what Settings shows, `plugin check`).
- `settings.json` gains `plugins` and `plugin_settings`.
- `src/lib/settings/SettingsDialog.svelte`, `src/lib/plugins/FieldsForm.svelte` (forms drawn
  from `[[settings]]`; tab setups use it in step 2).
- Tested: `cargo test` (105), `npm test`, `svelte-check`. In the dev app: installed a plugin
  from a local git repository (latest tag taken), its button showed in the Toolkit, a new tag
  was found on start and the update installed, settings saved, a broken manifest showed its
  problem. `thumbdeck plugin check` on a good and a broken plugin (exit 0 / 1).

### Step 2: the page API, and Git as a plugin

- `plugins/frame.rs` (the `plugin://` scheme, only files inside the plugin; the colors, kit and
  API put first in each page), `plugins/frame/api.js` (`window.thumbdeck`),
  `plugins/frame/kit.css`, `plugins/api.rs` (files, exec, storage, settings), `plugins/log.rs`.
- `src/lib/theme.css`: the colors, shared by the page and the frames.
- `src/lib/plugins/PluginFrame.svelte` (calls, events, keys, help), `PluginSetupForm.svelte`,
  `frame.ts`. Plugin tabs in the `+` menu, kept alive once shown, unloaded after 10 minutes
  hidden; badges on tab titles.
- Plugins repo (`src-tauri/toolkits`, branch `plugins`): `plugins/git` (the Git tab in plain JS,
  with `node --test` tests), `examples/todos`, `packages/plugin` (`@thumbdeck/plugin` types).
- The built-in Git tab is gone (Rust module, Svelte tab, its keymap and commands).
- Tested: `cargo test` (110), `npm test` (31), `svelte-check`, the Git plugin's tests (6), the
  types with `tsc --strict`. In the dev app: the TODOs example (exec, setup, badge, keys, `?`
  help from the manifest, setup form, a setup change reaching the page without a reload), and
  the Git plugin added from the `+` menu: branch line, changes and diffs, commits, the review
  page loading its diff from the plugin; a turned-off plugin's tab shows why, with Remove.
- Not in yet (later steps): `setup_page`, `ui.openPage` / `close` / `status` / `mood`, the
  backend.

### Step 3: the backend, and the packs as plugins

- `plugins/backend.rs`: started on first use, JSON lines, `initialize` / `actions` / `shutdown`,
  its requests (events, `ui.*`, storage, `actions.refresh`), stderr and stray prints to the log,
  restarts (not after three crashes in a minute), stopped on turn-off / update / remove and on
  quit. Frames call it with `td.backend.call`; its events reach them.
- Toolkit buttons from backends (in the background, cached until a watched file changes or a
  refresh); `[[generate]]` command sources work now (2 seconds, cached).
- `[backend] install` after installing and updating.
- The packs are plugins (`plugins/cargo`, `compose`, `django`, `go`, `gradle`, `make`, `npm`,
  `python`, each with a README); `packs/`, `packs.rs`, the bundling in `build.rs`,
  `update_packs` and **+ › Update toolkit packs** are gone. Plugins can give projects an `.svg`
  icon.
- Plugins repo: `packages/backend` (`@thumbdeck/backend`, with a test talking to a real
  backend), `examples/tasks` (a backend's buttons, a page calling it, events).
- Tested: `cargo test` (112, the backend with sh scripts: calls, errors, timeouts, its own
  requests, crashes), the helper's test, `npm test`, `svelte-check`. In the dev app: the
  Django and Python plugins give the old buttons and icon; the Tasks example's backend
  buttons show (without holding up the Toolkit), its tab lists them through the backend, a
  task run from the tab streams its output there and shows in Running, the backend's event
  arrives; closing the app stops the backend.

### Step 4: Pull requests, Agents and Logs as plugins

- The avatar hears plugins (`td.ui.mood` from frames and backends, added up); the page's own
  Claude polling is gone.
- `plugins/prs` (plain JS, `gh`; its demo data in `testdata/`), `plugins/agents` (Svelte UI, a
  Node backend reading Claude Code's files, `autostart` for the avatar), `plugins/logs` (Svelte
  viewer and setup page, a Node backend with the parser, sections and outline). Their Rust and
  Svelte tests were ported to `node --test` (the plugins repo: `npm test`, 50 tests).
- The plugins repo can build Svelte pages: `npm run build` (Vite), into each plugin's `dist/`.
- thumbdeck: `setup_page`, the `json` field type, `td.tmux(…, { show })`, `[backend] autostart`,
  frames may use the clipboard. `src/lib/extensions`, `src-tauri/src/extensions`, their
  commands and keymaps are gone.
- Tested: `cargo test` (70), `npm test` (19), `svelte-check`, the plugins repo's tests (50),
  `plugin check` on each. In the dev app: PRs (demo repo: list, details, review page from its
  test diff), Agents (this repo's real sessions, a conversation, keys switching keymaps), Logs
  (files and preview, a log, an entry, back, its setup page in the dialog).
- Not driven in the dev app: typing into a setup page (my test keys can't reach a field
  inside a frame while the window isn't the active one); saving from it is a short path
  (`td.setup.save` → the page's save). Worth a click when you try it.

### Step 5: the Plugins pane, views, panels, pages, the first start

- The Plugins pane below Projects (plugins with a `[view]`, their status); `Ctrl+p` gives it the
  keyboard; a view shows in the center, kept alive, following the selected project.
- Panels under the Toolkit (app or project scope, as tall as their page or a number of lines),
  pages over the whole window (`td.ui.openPage`, closed with q / Esc or `td.ui.close`).
- The catalog (`catalog.toml` in the plugins repo, tested against the plugins), the first
  start's offer, and the catalog in Settings; ↑ ↓ and Inspect in Settings.
- Reload on change for linked plugins.
- Plugins repo: `examples/pomodoro` (a view with a status, an app panel, a page, a setting,
  storage shared by two frames, a notification).
- Tested: `cargo test` (71), `npm test` (19), `svelte-check`, the plugins repo's tests. In the dev
  app with a fresh profile: the first start offered the 12 official plugins and installed them
  (from the local repo); the Toolkit and icon came back. Pomodoro: the pane and its status,
  `Ctrl+p` and Enter to its view, its keys, the panel ticking along, the history page and Esc
  back, an edit to its file reloading it, and a 1-minute session ending (history saved).
