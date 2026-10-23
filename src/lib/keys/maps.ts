// Every place's keys, in one file: one look shows them all. The handlers switch on the
// actions; the ? help is drawn from these lists and the status bar names who has the keyboard. Changing a key: here only.
// Rule 1: a vim key means what it means in vim. Rule 2: everything else is behind Space (the
// shared letters in RULES) or on the : line.
import { bind, HELP, MOVE, PAGES, type Keymap } from "./keys.ts";

// ------------------------------------------------------------ the main page
// The main page is panes, like nvim windows: Ctrl+w h/j/k/l (or Ctrl+h/j/k/l) moves between
// them, and the one with the keyboard has its own keys (below MAIN). MAIN's work in every
// pane, and in plugins' tabs and views too.
export const MAIN: Keymap = {
  name: "THUMBDECK",
  bindings: [
    bind(["Ctrl+h", "Ctrl+l", "Ctrl+w h", "Ctrl+w l"], "pane-side", "the pane to the left / right"),
    bind(["Ctrl+j", "Ctrl+k", "Ctrl+w j", "Ctrl+w k"], "pane-updown", "the pane below / above"),
    bind("Ctrl+w o", "wide", "wide: only the center (again: back)"),
    bind("Space Space", "toolkit", "the Toolkit: then the button's letter"),
    bind("Space 1–9", "page", "show a page (1 Overview, 2 README, then the project's A–Z)"),
    ...Array.from({ length: 9 }, (_, i) => bind(`Space ${i + 1}`, "page", "")),
    bind(["gt", "gT"], "cycle-page", "the next / previous page"),
    bind("]r", "next-run", "the next run's output"),
    bind("[r", "previous-run", "the previous run's output"),
    bind("Space s", "stop", "stop the shown run"),
    bind("Space a", "add", "add your own action"),
    bind("Space ,", "settings", "settings: plugins"),
    bind(":", "command", "the command line (:settings, :wide, :q …)"),
    bind("Ctrl+p", "plugins", "the Plugins pane"),
    bind("Escape", "leave", "close menus and dialogs, back to the projects"),
    HELP,
  ],
};

/** Of MAIN, what works while a plugin's tab or view has the keyboard (the rest is the plugin's) */
export const GLOBAL_ACTIONS = new Set(["pane-side", "pane-updown", "wide", "toolkit", "page", "cycle-page", "next-run", "previous-run", "settings", "command", "plugins"]);

/** The projects (on the left) */
export const PROJECTS: Keymap = {
  name: "PROJECTS",
  bindings: [
    ...MOVE.map((b) => ({ ...b, does: b.does === "down" ? "projects" : b.does })),
    bind("l", "open", "the project's center (the keyboard goes there)"),
    bind("Enter", "main", "open the project in tmux"),
    bind("/", "search", "filter the projects"),
    bind("f{x}", "find-letter", "the next project starting with that letter"),
    bind("F{x}", "find-letter-back", "the previous one"),
    bind("o", "new", "add a project folder"),
    bind("dd", "delete", "hide the project (or remove one you added)"),
    bind("Space p", "pin", "pin the project"),
    bind("Space t", "terminal", "a terminal in it (its tmux session)"),
    bind("R", "refresh", "look for projects again"),
  ],
};

/** The center while it shows the README (a plugin tab has its own keys) */
export const CENTER: Keymap = {
  name: "README",
  bindings: [
    bind(["j", "ArrowDown"], "down", "scroll down"),
    bind(["k", "ArrowUp"], "up", "scroll up"),
    ...PAGES,
    bind("gg", "first", "top"),
    bind("G", "last", "bottom"),
    bind("/", "search", "search the text"),
    bind("n", "next-match", "the next match"),
    bind("N", "previous-match", "the previous match"),
    bind("q", "leave", "back to the projects"),
  ],
};

/** A run's output in the center: scrolled like the README, and copied the vim way */
export const OUTPUT: Keymap = {
  name: "OUTPUT",
  bindings: [
    ...CENTER.bindings,
    bind("v", "visual", "select lines, from the last one (j / k extend, y copies)"),
    bind("V", "visual-line", "the same"),
    bind(["yy", "Y"], "yank", "copy the last line"),
    bind("Space y a", "yank-all", "copy the whole output"),
    bind("dd", "delete", "stop it"),
  ],
};

/** Lines selected in a run's output (after v / V) */
export const OUTPUT_VISUAL: Keymap = {
  name: "VISUAL",
  bindings: [
    ...MOVE,
    ...PAGES,
    bind(["y", "Y"], "yank", "copy the selected lines"),
    bind("v", "visual", "drop the selection"),
    bind("V", "visual-line", "drop the selection"),
    bind(["q", "Escape"], "leave", "drop the selection"),
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
    bind("dd", "delete", "hide the card in this project (hidden ones are listed below the cards)"),
    bind("q", "leave", "back to the projects"),
  ],
};

/** The Running list (on the right) */
export const RUNNING: Keymap = {
  name: "RUNNING",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "show its output"),
    bind("dd", "delete", "stop it"),
    bind("Space s", "stop", "stop it"),
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
    bind(["i", "a"], "edit", "edit your action"),
    bind("o", "new", "add your own action"),
    bind("dd", "delete", "remove your action (asks first)"),
    bind("q", "leave", "back to the projects"),
  ],
};

/** After Space (or Space Space): a letter runs its Toolkit button */
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
  bindings: [bind("Tab", "next-list", "next field"), bind(["Ctrl+s"], "save", "save (or :w)"), bind("Escape", "leave", "close")],
};

/** The : line */
export const COMMAND: Keymap = {
  name: "COMMAND",
  bindings: [bind("Tab", "next-list", "complete"), bind("Enter", "open", "run it"), bind("Escape", "leave", "never mind")],
};

// ------------------------------------------------------------ the review page
export const REVIEW: Keymap = {
  name: "REVIEW",
  bindings: [
    ...MOVE,
    ...PAGES,
    bind("]c", "next-change", "the next change (past the last: the next file)"),
    bind("[c", "previous-change", "the previous change"),
    bind("]t", "next-thread", "the next thread (in every file)"),
    bind("[t", "previous-thread", "the previous thread"),
    bind("]f", "next-file", "the next file"),
    bind("[f", "previous-file", "the previous file"),
    bind("cc", "change", "comment on the line"),
    bind("v", "visual", "select lines (j / k extend), then c comments or y copies"),
    bind("V", "visual-line", "the same"),
    bind(["yy", "Y"], "yank", "copy the line"),
    bind("Enter", "press", "reply to the thread · show the hidden lines"),
    bind("zo", "fold-open", "show the hidden lines here"),
    bind("zR", "unfold-all", "the whole file"),
    bind("zM", "fold-all", "only the changes"),
    bind("x", "tick", "on a thread: resolve it (or show a resolved one); elsewhere: the file is viewed, on to the next"),
    bind("Ctrl+w v", "split", "side by side ↔ one column"),
    bind("Space i", "info", "about the pull request"),
    bind("Space A", "approve", "approve (finish your review; or :approve)"),
    bind("Space R", "request-changes", "request changes (finish your review; or :request)"),
    bind("Space P", "post", "post your review (or :w)"),
    bind("-", "files", "the file list: j / k show each file (/ there filters them)"),
    bind("/", "search", "search the changes in every file (+word: added lines, -word: removed)"),
    bind("n", "next-match", "the next match (on to the next files)"),
    bind("N", "previous-match", "the previous match"),
    bind("gx", "outside", "open it in the browser"),
    bind("R", "refresh", "read the changes and comments again"),
    bind(["q", "Escape"], "leave", "clear the search, else back"),
    HELP,
  ],
};

/** Lines selected on the review page (after v / V) */
export const REVIEW_VISUAL: Keymap = {
  name: "VISUAL",
  bindings: [
    ...MOVE,
    ...PAGES,
    bind("c", "comment", "comment on the selected lines"),
    bind(["y", "Y"], "yank", "copy the selected lines"),
    bind("Enter", "comment", "comment on them"),
    bind("v", "visual", "drop the selection"),
    bind("V", "visual-line", "drop the selection"),
    bind(["q", "Escape"], "leave", "drop the selection"),
    HELP,
  ],
};

/** The review page's file list (after -): moving shows each file */
export const REVIEW_FILES: Keymap = {
  name: "FILES",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "into the file's changes"),
    bind("x", "tick", "mark the file viewed / not"),
    bind("/", "search", "filter the files"),
    bind(["h", "-", "q", "Escape", "Backspace"], "leave", "back to the changes"),
    HELP,
  ],
};

/** Writing a comment on the review page */
export const REVIEW_COMMENT: Keymap = {
  name: "COMMENT",
  bindings: [
    bind(["Ctrl+Enter", "Ctrl+s"], "send", "add it to your review (a reply: send it)"),
    bind("Ctrl+g", "suggest", "suggest a change to the lines"),
    bind("Escape", "leave", "cancel"),
  ],
};

/** Finishing a review */
export const REVIEW_POST: Keymap = {
  name: "POST",
  bindings: [
    bind(["1", "2", "3"], "pick", "comment / approve / request changes"),
    bind(["j", "ArrowDown"], "down", "the next choice"),
    bind(["k", "ArrowUp"], "up", "the previous choice"),
    bind(["Ctrl+Enter", "Ctrl+s"], "send", "post the review"),
    bind("Escape", "leave", "not yet"),
  ],
};

/** The Plugins pane (below the projects) */
export const PLUGINS: Keymap = {
  name: "PLUGINS",
  bindings: [
    ...MOVE,
    bind(["l", "Enter"], "open", "show the plugin's view (its keys go to it)"),
    bind("f{x}", "find-letter", "the next plugin starting with that letter"),
    bind("F{x}", "find-letter-back", "the previous one"),
    bind("q", "leave", "back to the projects"),
  ],
};

export const ALL: Keymap[] = [MAIN, PROJECTS, PLUGINS, CENTER, OUTPUT, OUTPUT_VISUAL, RUNNING, KIT, OVERVIEW, TOOLKIT, FILTER, DIALOG, COMMAND, REVIEW, REVIEW_VISUAL, REVIEW_FILES, REVIEW_COMMENT, REVIEW_POST];

/**
 * Keys that changed meaning in thumbdeck 1.0: pressed where they don't do anything any more,
 * they say where the action went (so a habit can't delete anything). Until 1.1.
 */
export const MOVED: Record<string, string> = {
  d: "half a page down is Ctrl+d now (dd deletes)",
  u: "half a page up is Ctrl+u now (u undoes)",
  g: "the top is gg now",
  z: "wide is Ctrl+w o now (z starts folds: za, zR, zM)",
  r: "refresh is R now",
  s: "stop is Space s now",
  v: "the review is Enter or gd now (v selects)",
  o: "open outside is gx now (o makes a new item)",
  p: "pin is Space p now",
  e: "edit is i now",
  c: "comment is cc now",
  t: "the next thread is ]t now",
  "]": "the next run is ]r (]c change, ]t thread, ]f file)",
  "[": "the previous run is [r ([c change, [t thread, [f file)",
  ",": "settings is Space , now (or :settings)",
  "Ctrl+b": "the next page is gt now (Ctrl+b scrolls a page up)",
  a: "add an action is Space a now",
  ...Object.fromEntries(Array.from({ length: 9 }, (_, i) => [String(i + 1), `page ${i + 1} is Space ${i + 1} now (digits are counts: 5j)`])),
};
