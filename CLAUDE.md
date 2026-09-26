# thumbdeck

Desktop app (Tauri 2 + Svelte 5, Linux and macOS): projects on the left, the selected one's
README / tabs / run output in the center (tabs at the bottom), Running + Toolkit on the right.
The user reads everything in plain English; the README is the user-facing documentation.

## Map

- `src-tauri/src/lib.rs`: Tauri commands and `details()` (what the page gets for a project)
- `packs.rs` + `actions.rs`: toolkit packs (TOML, format in the `src-tauri/toolkits` submodule's
  SPEC.md), compiled in by `build.rs`; `$THUMBDECK_TOOLKITS` points the build at another checkout
- `terminal.rs`: tmux (one session per project: nvim + a shell; `run_in_window` for tmux actions)
- `runner.rs`: running actions (login-shell env, process groups, finish notifications)
- `settings.rs`: `~/.config/com.nozeren.thumbdeck/settings.json` (custom actions, hidden
  actions, extension tabs per project)
- `updater.rs`: self-update from github.com/Nozeren/thumbdeck-releases (signed; public key
  `updater.pub`, private key `~/.tauri/thumbdeck.key`, never in the repo)
- `extensions/`: built-in tab extensions, one registry each side: `AVAILABLE` in
  `extensions/mod.rs` and `src/lib/extensions/index.ts`. `logs/` (log viewer), `agents/`
  (Claude Code sessions from `~/.claude/projects/<path with non-alphanumerics as ->/`, subagents
  in `<session>/subagents/agent-<id>.jsonl` + `.meta.json`; undocumented format: read defensively)
- `src/routes/+page.svelte`: the whole page (keys in `onKey`, help dialog, styles; shared dialog
  and button styles are `:global` there)

## Checks (run all before saying something works)

```sh
cd src-tauri && cargo test                  # Rust (tests live next to the code)
npm test                                    # pure TS logic: node --test src/**/*.test.ts
npx svelte-check --tsconfig ./tsconfig.json # types
```

Test data lives in `testdata/` folders next to the code; strip anything personal (paths, email)
from real files first.

## Trying the app

- `npm run tauri dev` with `XDG_CONFIG_HOME=<scratch dir>` so the user's settings stay untouched;
  a demo project with its own settings.json there is the usual setup.
- Drive it (Hyprland) only through the dev window, found by its process (`target/debug/thumbdeck`),
  never the installed app (`~/.local/bin/thumbdeck`): `hyprctl dispatch sendshortcut ", <key>, address:<addr>"`;
  screenshot with `alterzorder top` then `grim -g`, and `alterzorder bottom` after.
- Never send Enter unless a button has keyboard focus: Enter opens the project in the user's
  real tmux. Space + letter runs Toolkit buttons; number keys switch tabs.
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
