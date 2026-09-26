// Every place's keys, in one file: one look shows them all. The handlers switch on the
// actions; the ? help is drawn from these lists and the status bar names who has the keyboard. Changing a key: here only.
import { bind, HELP, LEAVE, MOVE, SETUP, type Keymap } from "./keys.ts";

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

// ------------------------------------------------------------ Logs tab
export const LOGS_FILES: Keymap = {
  name: "LOGS",
  bindings: [...MOVE, bind(["l", "Enter"], "open", "open the log"), bind("r", "refresh", "look for logs again"), SETUP, LEAVE, HELP],
};

export const LOGS: Keymap = {
  name: "LOG",
  bindings: [
    ...MOVE,
    bind(["l", "Enter", "Space", "ArrowRight"], "open", "open a section / a line's full entry"),
    bind(["h", "ArrowLeft"], "close", "close a section (on a line: its section)"),
    bind(["d", "Ctrl+d"], "page-down", "half a page down"),
    bind(["u", "Ctrl+u"], "page-up", "half a page up"),
    bind(["Ctrl+f", "PageDown"], "full-down", "a page down"),
    bind(["Ctrl+b", "PageUp"], "full-up", "a page up"),
    bind(["e", "E"], "error", "next / previous ERROR"),
    bind("/", "search", "search"),
    bind(["n", "N"], "match", "next / previous match"),
    bind("t", "tail", "live tail on / off"),
    bind("f", "levels", "show / hide levels: then D I W E, or A for all"),
    bind(["y", "Y"], "copy", "copy the message (a section: its title) / the entry as JSON"),
    bind("r", "refresh", "read the log again"),
    bind(["Backspace", "-"], "back", "back to the files"),
    SETUP,
    LEAVE,
    HELP,
  ],
};

/** After f in a log */
export const LOGS_LEVELS: Keymap = {
  name: "LEVELS",
  bindings: [
    bind("D", "DEBUG", "show / hide DEBUG"),
    bind("I", "INFO", "show / hide INFO"),
    bind("W", "WARNING", "show / hide WARNING"),
    bind("E", "ERROR", "show / hide ERROR"),
    bind("A", "all", "show all"),
    bind("Escape", "leave", "never mind"),
  ],
};

/** A line's full entry */
export const LOGS_ENTRY: Keymap = {
  name: "ENTRY",
  bindings: [
    ...MOVE,
    bind(["l", "Enter", "Space"], "open", "open / close a value"),
    bind("h", "close", "close (on a value: its parent)"),
    bind(["y", "Y"], "copy", "copy the value / the whole entry"),
    bind(["q", "Escape", "Backspace"], "leave", "close the entry"),
    HELP,
  ],
};

// ------------------------------------------------------------ Agents tab
export const AGENTS_LIST: Keymap = {
  name: "AGENTS",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "open: an agent or skill to start, a session's conversation"),
    bind("Tab", "next-list", "next list: agents → skills → sessions"),
    bind("Shift+Tab", "previous-list", "previous list"),
    bind("r", "refresh", "look again"),
    SETUP,
    LEAVE,
    HELP,
  ],
};

export const AGENTS_TRANSCRIPT: Keymap = {
  name: "CONVERSATION",
  bindings: [
    ...MOVE,
    bind(["d", "PageDown"], "page-down", "a page down"),
    bind(["u", "PageUp"], "page-up", "a page up"),
    bind(["l", "Enter", "Space"], "open", "a tool call: its details; ◇ Agent: its conversation"),
    bind(["h", "Backspace", "-"], "back", "back"),
    bind("r", "refresh", "read it again"),
    LEAVE,
    HELP,
  ],
};

export const AGENTS_START: Keymap = {
  name: "START",
  bindings: [
    bind(["i", "Enter"], "type", "type the task (Enter in the box starts Claude in tmux)"),
    bind(["h", "Backspace", "-"], "back", "back"),
    LEAVE,
    HELP,
  ],
};

// ------------------------------------------------------------ Pull requests tab
export const PRS: Keymap = {
  name: "PULL REQUESTS",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "review its changes"),
    bind("v", "review", "review its changes"),
    bind("o", "outside", "open in the browser (marks its 🔔 read)"),
    bind(["d", "PageDown"], "page-down", "scroll the details down"),
    bind(["u", "PageUp"], "page-up", "scroll the details up"),
    bind("r", "refresh", "refresh now"),
    SETUP,
    LEAVE,
    HELP,
  ],
};

// ------------------------------------------------------------ Git tab
export const GIT: Keymap = {
  name: "GIT",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "review: all changes (at this file), the commit, the stash"),
    bind("v", "review", "review (the same)"),
    bind("Tab", "next-list", "next list: changes → commits → branches"),
    bind("Shift+Tab", "previous-list", "previous list"),
    bind(["d", "PageDown"], "page-down", "scroll the diff down"),
    bind(["u", "PageUp"], "page-up", "scroll the diff up"),
    bind("r", "refresh", "refresh"),
    SETUP,
    LEAVE,
    HELP,
  ],
};

export const ALL: Keymap[] = [MAIN, TOOLKIT, FILTER, DIALOG, REVIEW, REVIEW_FILES, LOGS_FILES, LOGS, LOGS_LEVELS, LOGS_ENTRY, AGENTS_LIST, AGENTS_TRANSCRIPT, AGENTS_START, PRS, GIT];
