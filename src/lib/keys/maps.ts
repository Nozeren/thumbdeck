// Every place's keys, in one file: one look shows them all. The handlers switch on the
// actions; the ? help is drawn from these lists and the status bar names who has the keyboard. Changing a key: here only.
import { bind, HELP, MOVE, type Keymap } from "./keys.ts";

// ------------------------------------------------------------ the main page
export const MAIN: Keymap = {
  name: "THUMBDECK",
  bindings: [
    ...MOVE.map((b) => ({ ...b, does: b.does === "down" ? "projects" : b.does })),
    bind(["Enter", "o"], "outside", "open the project in tmux"),
    bind("Space", "toolkit", "toolkit: then the button's letter"),
    bind("1–9", "tabs", "show a tab (1: README), its keys go to it"),
    bind("/", "filter", "filter the projects"),
    bind(["]", "["], "run", "next / previous run's output"),
    bind("s", "stop", "stop the shown run"),
    bind("a", "add", "add a toolkit action"),
    bind("p", "pin", "pin the project"),
    bind("x", "remove", "hide the project (or remove one you added)"),
    bind("z", "wide", "wide: only the center (anywhere)"),
    bind(",", "settings", "settings: plugins"),
    bind("Ctrl+p", "plugins", "the Plugins pane: the plugins' own views"),
    bind("Escape", "leave", "close menus and dialogs"),
    HELP,
  ],
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

/** The Plugins pane (Ctrl+p) */
export const PLUGINS: Keymap = {
  name: "PLUGINS",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "show the plugin's view (its keys go to it)"),
    bind(["q", "Escape"], "leave", "back to the projects"),
    HELP,
  ],
};

export const ALL: Keymap[] = [PLUGINS, MAIN, TOOLKIT, FILTER, DIALOG, REVIEW, REVIEW_FILES];
