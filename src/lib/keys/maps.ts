// Every place's keys, in one file: one look shows them all. The handlers switch on the
// actions; the ? help is drawn from these lists and the status bar names who has the keyboard. Changing a key: here only.
import { bind, HELP, MOVE, type Keymap } from "./keys.ts";

// ------------------------------------------------------------ the main page
// The main page is panes, like nvim splits or tmux panes: Ctrl+h/j/k/l moves between them,
// and the one with the keyboard has its own keys (below MAIN). MAIN's work in every pane.
export const MAIN: Keymap = {
  name: "THUMBDECK",
  bindings: [
    bind(["Ctrl+h", "Ctrl+l"], "pane-side", "the pane to the left / right"),
    bind(["Ctrl+j", "Ctrl+k"], "pane-updown", "the pane below / above"),
    bind("Space", "toolkit", "toolkit: then the button's letter"),
    bind("1–9", "tabs", "show a tab (1: Overview, 2: README), its keys go to it"),
    bind(["]", "["], "run", "next / previous run's output"),
    bind("s", "stop", "stop the shown run"),
    bind("a", "add", "add a toolkit action"),
    bind("z", "wide", "wide: only the center (anywhere)"),
    bind(",", "settings", "settings: plugins"),
    bind("Ctrl+p", "plugins", "the Plugins pane"),
    bind("Ctrl+b", "prefix", "then n / p: the next / previous tab (as in tmux)"),
    bind("Escape", "leave", "close menus and dialogs, back to the projects"),
    HELP,
  ],
};

/** The projects (on the left) */
export const PROJECTS: Keymap = {
  name: "PROJECTS",
  bindings: [
    ...MOVE.map((b) => ({ ...b, does: b.does === "down" ? "projects" : b.does })),
    bind("l", "open", "the project's center (the keyboard goes there)"),
    bind(["Enter", "o"], "outside", "open the project in tmux"),
    bind("/", "filter", "filter the projects"),
    bind("p", "pin", "pin the project"),
    bind("x", "remove", "hide the project (or remove one you added)"),
  ],
};

/** The center while it shows the README or a run's output (a plugin tab has its own keys) */
export const CENTER: Keymap = {
  name: "CENTER",
  bindings: [
    bind(["j", "ArrowDown"], "down", "scroll down"),
    bind(["k", "ArrowUp"], "up", "scroll up"),
    bind(["d", "PageDown"], "page-down", "half a page down"),
    bind(["u", "PageUp"], "page-up", "half a page up"),
    bind("g", "first", "top"),
    bind("G", "last", "bottom"),
    bind("q", "leave", "back to the projects"),
  ],
};

/** The Overview (the center's first tab): its cards are a grid, so h / l move sideways. The
 *  card under the cursor gets its own keys too (a plugin's card: those in its plugin.toml). */
export const OVERVIEW: Keymap = {
  name: "OVERVIEW",
  grid: true,
  bindings: [
    ...MOVE.map((b) => ({ ...b, does: b.does === "down" ? "cards" : b.does })),
    bind(["h", "ArrowLeft"], "left", "left"),
    bind(["l", "ArrowRight"], "right", "right"),
    bind("Enter", "press", "open the card (Last runs: the Toolkit; a plugin's: its tab)"),
    bind("x", "hide", "hide the card in this project (hidden ones are listed below the cards)"),
    bind("q", "leave", "back to the projects"),
  ],
};

/** The Running list (on the right) */
export const RUNNING: Keymap = {
  name: "RUNNING",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "show its output"),
    bind("s", "stop-this", "stop it"),
    bind("q", "leave", "back to the projects"),
  ],
};

/** The Toolkit's buttons (on the right): a grid, so h / l move sideways */
export const KIT: Keymap = {
  name: "TOOLKIT",
  grid: true,
  bindings: [
    ...MOVE,
    bind(["h", "ArrowLeft"], "left", "left"),
    bind(["l", "ArrowRight"], "right", "right"),
    bind("Enter", "press", "run the button"),
    bind("e", "edit", "edit your action"),
    bind("q", "leave", "back to the projects"),
  ],
};

/** After Ctrl+b, as in tmux */
export const PREFIX: Keymap = {
  name: "Ctrl+b",
  bindings: [bind("n", "next-tab", "the next tab"), bind("p", "previous-tab", "the previous tab"), bind("Escape", "leave", "never mind")],
};

/** After Space: a letter runs its Toolkit button */
export const TOOLKIT: Keymap = {
  name: "TOOLKIT",
  bindings: [bind("a–z", "run", "run the button with that letter"), bind("Escape", "leave", "never mind")],
};

export const FILTER: Keymap = {
  name: "FILTER",
  bindings: [bind("Enter", "open", "go to the first match"), bind("Escape", "leave", "stop filtering")],
};

export const DIALOG: Keymap = {
  name: "DIALOG",
  bindings: [bind("Tab", "next-list", "next field"), bind("Escape", "leave", "close")],
};

// ------------------------------------------------------------ the review page
export const REVIEW: Keymap = {
  name: "REVIEW",
  bindings: [
    bind(["n", "N"], "change", "next / previous change (past the last: the next file)"),
    bind("-", "files", "the file list: pick another file with j / k and l"),
    bind("x", "viewed", "mark the file viewed, on to the next"),
    bind("s", "split", "side by side ↔ one column"),
    bind(["j", "ArrowDown"], "down", "scroll down"),
    bind(["k", "ArrowUp"], "up", "scroll up"),
    bind(["d", "Space", "PageDown"], "page-down", "a page down"),
    bind(["u", "PageUp"], "page-up", "a page up"),
    bind("g", "first", "top"),
    bind("G", "last", "bottom"),
    bind("r", "refresh", "read the changes again"),
    bind(["q", "Escape", "h", "Backspace"], "leave", "back"),
    HELP,
  ],
};

/** The review page's file list (after -) */
export const REVIEW_FILES: Keymap = {
  name: "FILES",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "open the file's changes"),
    bind("x", "viewed", "mark the file viewed / not"),
    bind(["h", "-", "q", "Escape", "Backspace"], "leave", "back to the changes"),
    HELP,
  ],
};

/** The Plugins pane (below the projects) */
export const PLUGINS: Keymap = {
  name: "PLUGINS",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "show the plugin's view (its keys go to it)"),
    bind("q", "leave", "back to the projects"),
  ],
};

export const ALL: Keymap[] = [MAIN, PROJECTS, PLUGINS, CENTER, RUNNING, KIT, OVERVIEW, PREFIX, TOOLKIT, FILTER, DIALOG, REVIEW, REVIEW_FILES];
