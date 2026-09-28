# thumbdeck

Desktop app (Tauri 2 + Svelte 5, Linux and macOS): projects (and the Plugins pane) on the left,
the selected one's README / tabs / run output in the center (tabs at the bottom), Running +
Toolkit + plugin panels on the right. Tabs, Toolkit buttons, panels, views and pages all come
from plugins.
The user reads everything in plain English; the README is the user-facing documentation.

## Map

- `src-tauri/src/lib.rs`: Tauri commands and `details()` (what the page gets for a project)
- `src-tauri/src/plugins/`: plugins (the contract is SPEC.md in the `src-tauri/toolkits` submodule, the
  plugins repo; `docs/plugin-work.md` has how it was built and why). `manifest.rs` (plugin.toml
  and its checks), `install.rs` (git URL at the latest tag, linking a folder, updates),
  `detect.rs` + `toolkit.rs` (Toolkit buttons), `frame.rs` (the `plugin://` scheme; puts
  `frame/api.js` = `window.thumbdeck` and `frame/kit.css` in each page), `api.rs` (files,
  exec, storage for frames), `backend.rs` (backends: JSON lines), `catalog.rs`, `keys.rs` (the
  key rules again: a test keeps them equal to `keys.ts` and `@thumbdeck/check`), `official.rs`
  (tests that the official plugins give the old buttons)
- `terminal.rs`: tmux (one session per project: nvim + a shell; `run_in_window` for tmux actions)
- `runner.rs`: running actions (login-shell env, process groups, finish notifications)
- `settings.rs`: `~/.config/com.nozeren.thumbdeck/settings.json` (custom actions, hidden
  actions, installed plugins and their settings, plugin tabs per project). Plugins themselves
  live in `~/.local/share/thumbdeck/plugins` (`$XDG_DATA_HOME`)
- `updater.rs`: self-update from github.com/Nozeren/thumbdeck-releases (signed; public key
  `updater.pub`, private key `~/.tauri/thumbdeck.key`, never in the repo)
- `src/lib/plugins/`: `PluginFrame.svelte` (a plugin page in an iframe: answers its calls,
  sends it events and key actions), setup forms, the first start's catalog dialog;
  `src/lib/settings/SettingsDialog.svelte` (Settings › Plugins)
- `src-tauri/toolkits/` (the plugins repo): `plugins/` (git, logs, agents, prs, and the
  Toolkit ones), `examples/`, `packages/` (`@thumbdeck/plugin` types, `@thumbdeck/backend`,
  `@thumbdeck/check`), `catalog.toml`, GUIDE.md, SPEC.md. Its own checks: `npm test`,
  `npm run check`, `npm run build` (the Svelte pages of agents and logs into `dist/`, committed)
- `src/lib/keys/`: every place's keys in `maps.ts` (handlers switch on its actions; the ? help
  is drawn from it); `RULES` in `keys.ts` (a key means the same everywhere) is checked by
  `npm test`. `src/lib/StatusBar.svelte`: the bottom line (who has the keyboard, branch, messages)
- `src/lib/review/`: the review page (diff parsing in `diff.ts`), opened by Git and PR tabs
- `src/routes/+page.svelte`: the whole page (keys in `onKey`, help dialog, styles; shared dialog
  and button styles are `:global` there)

## Checks (run all before saying something works)

```sh
cd src-tauri && cargo test                  # Rust (tests live next to the code)
npm test                                    # pure TS logic: node --test src/**/*.test.ts
npx svelte-check --tsconfig ./tsconfig.json # types
cd src-tauri/toolkits && npm test && npm run check   # the plugins, when they changed
```

Test data lives in `testdata/` folders next to the code; strip anything personal (paths, email)
from real files first.

## Trying the app

- `npm run tauri dev` with `XDG_CONFIG_HOME=<scratch dir>` and `XDG_DATA_HOME=<scratch dir>` so
  the user's settings and plugins stay untouched; a demo project with its own settings.json there
  is the usual setup. Plugins are linked there from `src-tauri/toolkits` (they reload as you
  save; `.taurignore` keeps those edits from restarting the app).
- Keys sent with sendshortcut don't reach a field inside a plugin frame while the window isn't
  active: check typing in plugin pages by hand.
- Drive it (Hyprland) only through the dev window, found by its process (`target/debug/thumbdeck`),
  never the installed app (`~/.local/bin/thumbdeck`): `hyprctl dispatch sendshortcut ", <key>, address:<addr>"`;
  screenshot with `alterzorder top` then `grim -g`, and `alterzorder bottom` after.
- Never send Enter or `o` on the main page unless a button has keyboard focus: they open the
  project in the user's real tmux. Space + letter runs Toolkit buttons; number keys switch tabs.
- An unstyled page in dev is vite's stuck CSS cache (it serves the raw .svelte source as the
  CSS): `touch src/routes/+page.svelte`, and if that doesn't help, restart `npm run tauri dev`.
- Keep screenshots and command output few and small; they fill the context fast.

## Shell pitfalls (zsh)

- `=word` expands as a command path: quote it (`--proto '=https'`, `tmux -t '=name'`).
- `pgrep -f` / `pkill -f` match their own command line; find processes with
  `ps -eo pid,args | grep '[t]arget/debug/thumbdeck'` and kill by PID.

## Releasing

`scripts/release.sh X.Y.Z` (sets the version in 5 files, commits, tags), then
`git push origin main vX.Y.Z`; GitHub Actions builds, signs and publishes (secrets
`RELEASES_TOKEN`, `TAURI_SIGNING_PRIVATE_KEY`). The user reinstalls with `./install.sh`.

## Working with the user

- Ask before big design choices; build in steps and test each one.
- Commit and push only when asked; the user runs `install.sh` themselves.
- Comments and UI text: short, plain sentences saying what and why.
