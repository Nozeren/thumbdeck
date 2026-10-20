// Keys, the same way everywhere: each place (the main page, a tab in one of its modes, the
// review page) lists its keys once, as a Keymap. Its key handler looks keys up there, and the
// ? help is drawn from it, so they can't drift apart. RULES says what a key means wherever
// it's used (keys.test.ts checks every keymap against it).
//
// Two rules: a vim key means what it means in vim (j, gg, Ctrl+d, dd, yy, cc, za, ]c, …), and
// everything else is behind Space (letters shared by every place) or on the : line. Keys can be
// sequences ("gg", "]c", "Ctrl+w o", "Space y j"), take a letter ("f{x}", "m{x}") and a count in
// front ("5j", "3dd"); a KeyReader puts presses together into actions.

export interface Binding {
  /** Key names: "j", "Enter", "Ctrl+d", or sequences of them: "gg", "]c", "Ctrl+w o", "Space s", "f{x}" */
  keys: string[];
  /** What it does, in a few words */
  does: string;
  /** What the place runs (its handler switches on this) */
  action: string;
}

export interface Keymap {
  /** Who has the keyboard: shown at the start of the status bar ("GIT", "REVIEW") */
  name: string;
  bindings: Binding[];
  /** A grid of buttons (the Toolkit): h / l move left / right there */
  grid?: boolean;
}

/**
 * Rule 1 and the shared Space letters: key -> the action it must run wherever it's bound.
 * Places may leave a key out, never give it another meaning.
 */
export const RULES: Record<string, string> = {
  j: "down", k: "up", gg: "first", G: "last",
  "Ctrl+d": "page-down", "Ctrl+u": "page-up", "Ctrl+f": "full-down", "Ctrl+b": "full-up", zz: "center",
  l: "open", Tab: "next-list", "Shift+Tab": "previous-list",
  "/": "search", n: "next-match", N: "previous-match",
  "f{x}": "find-letter", "F{x}": "find-letter-back",
  "m{x}": "mark", "'{x}": "jump",
  v: "visual", V: "visual-line", yy: "yank", Y: "yank", p: "paste", P: "paste-before", x: "tick",
  dd: "delete", cc: "change", o: "new", O: "new-above", i: "edit", a: "edit", r: "replace",
  u: "undo", "Ctrl+r": "redo",
  za: "fold-toggle", zo: "fold-open", zc: "fold-close", zR: "unfold-all", zM: "fold-all",
  "]c": "next-change", "[c": "previous-change", "]t": "next-thread", "[t": "previous-thread",
  "]f": "next-file", "[f": "previous-file", "]e": "next-error", "[e": "previous-error",
  "]r": "next-run", "[r": "previous-run",
  gd: "diff", gx: "outside", R: "refresh",
  "Ctrl+w v": "split",
  q: "leave", Escape: "leave", "?": "help",
  "Space s": "stop", "Space p": "pin", "Space f": "filter", "Space o": "order", "Space h": "hidden",
  "Space i": "info", "Space e": "editor", "Space t": "terminal", "Space w": "watch", "Space /": "search-wide",
  "Space a": "add", "Space S": "setup",
};

/** Plugins written for plugin API 1 keep the keys they were made with */
export const RULES_API1: Record<string, string> = {
  j: "down", k: "up", g: "first", G: "last",
  d: "page-down", u: "page-up",
  l: "open",
  Tab: "next-list", "Shift+Tab": "previous-list",
  v: "review", o: "outside", r: "refresh",
  q: "leave", Escape: "leave",
  S: "setup", "?": "help",
};

/** Keys thumbdeck keeps for itself, wherever the keyboard is (a plugin can't bind them) */
export const RESERVED = [
  "Space", "Space Space", "Space ,", ...Array.from({ length: 9 }, (_, i) => `Space ${i + 1}`),
  "gt", "gT", ":", ".", ";", ",", "Ctrl+p", "Ctrl+h", "Ctrl+j", "Ctrl+k", "Ctrl+l",
  "Ctrl+w o", "Ctrl+w h", "Ctrl+w j", "Ctrl+w k", "Ctrl+w l",
  ...Array.from({ length: 10 }, (_, i) => String(i)),
];

/** Changes . repeats */
const CHANGES = new Set(["delete", "tick", "change", "replace", "paste", "paste-before", "new", "new-above"]);

/** What h / l do in a grid instead of their rule */
const GRID_MOVES: Record<string, string> = { h: "left", l: "right" };

const NAMED = new Set(["Enter", "Escape", "Space", "Tab", "Backspace", "Delete", "Home", "End", "PageDown", "PageUp", "ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Insert"]);

/** A key press as a name: "j", "G", "Enter", "Space", "Shift+Tab", "Ctrl+d" */
export function keyName(e: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): string {
  const key = e.key === " " ? "Space" : e.key;
  // Shift is part of letters and symbols already ("G", "?"); only named keys spell it out
  const shift = e.shiftKey && key.length > 1 && key !== "Space" ? "Shift+" : "";
  const mods = `${e.ctrlKey ? "Ctrl+" : ""}${e.altKey ? "Alt+" : ""}${e.metaKey ? "Meta+" : ""}`;
  return mods + shift + key;
}

/** A binding's key as the presses it takes: "gg" -> g g, "Ctrl+w o" -> Ctrl+w o, "f{x}" -> f {x} */
export function tokens(key: string): string[] {
  const out: string[] = [];
  for (const part of key.split(" ")) {
    if (!part) continue;
    if (part.length === 1 || NAMED.has(part) || part.includes("+") && part.length > 1 || /^F\d+$/.test(part)) out.push(part);
    else for (let i = 0; i < part.length; i++) {
      if (part.startsWith("{x}", i)) (out.push("{x}"), (i += 2));
      else out.push(part[i]);
    }
  }
  return out;
}

/** Keys written for the help only ("1–9", "a–z"): never matched */
const shownOnly = (key: string) => key.includes("–");

/** One finished press: what to run, the keys that made it, the count before it and a letter it took */
export interface Press {
  action: string;
  key: string;
  count: number | null;
  arg?: string;
}

export type Fed =
  /** then: the key after a waiting shorter binding, read afresh */
  | { kind: "action"; press: Press; then?: Fed }
  | { kind: "wait"; pending: string }
  | { kind: "none"; cancelled: string | null };

/**
 * Puts key presses together into actions, the vim way: a count in front, sequences, a letter
 * after f / m / ', . to repeat the last change, ; and , to repeat the last f. One per place.
 */
export class KeyReader {
  private pending: string[] = [];
  private count = "";
  private fallback: Press | null = null;
  private lastChange: Press | null = null;
  private lastFind: Press | null = null;

  /** What's been typed so far (for the status bar, like vim's showcmd): "3d", "g", "Space" */
  get shown(): string {
    return this.count + this.pending.map((t) => (t === "Space" ? "Space " : t)).join("");
  }
  get waiting(): boolean {
    return this.pending.length > 0 || this.count !== "";
  }
  /** The bindings that can come next (for the Space menu): [the next key, the binding] */
  next(map: Keymap): [string, Binding][] {
    const out: [string, Binding][] = [];
    for (const b of map.bindings) for (const k of b.keys) {
      if (shownOnly(k)) continue;
      const t = tokens(k);
      if (t.length > this.pending.length && this.pending.every((p, i) => t[i] === p)) out.push([t.slice(this.pending.length).join(" "), b]);
    }
    return out;
  }

  /** Time's up (vim's timeoutlen): a waiting shorter binding runs; null when nothing waits */
  flush(): Press | null {
    const f = this.fallback;
    if (!f) return null;
    this.reset();
    this.remember(f);
    return f;
  }

  reset() {
    this.pending = [];
    this.count = "";
    this.fallback = null;
  }

  /** Feed one key press (its name); counts only where the place takes them */
  feed(map: Keymap, name: string, o: { counts?: boolean } = {}): Fed {
    if (["Control", "Shift", "Alt", "Meta"].includes(name.split("+").pop()!)) return { kind: "wait", pending: this.shown };
    if (name === "Escape" && this.waiting) {
      const was = this.shown;
      this.reset();
      return { kind: "none", cancelled: was };
    }
    if (o.counts !== false && this.pending.length === 0 && /^[0-9]$/.test(name) && (name !== "0" || this.count)) {
      this.count += name;
      return { kind: "wait", pending: this.shown };
    }
    const count = this.count ? Number(this.count) : null;
    if (this.pending.length === 0) {
      if (name === "." && this.lastChange) return this.done({ ...this.lastChange, count: count ?? this.lastChange.count });
      if ((name === ";" || name === ",") && this.lastFind) {
        const back = (this.lastFind.action === "find-letter-back") !== (name === ",");
        return this.done({ ...this.lastFind, action: back ? "find-letter-back" : "find-letter", count });
      }
    }
    const typed = [...this.pending, name];
    let exact: Press | null = null;
    let longer = false;
    for (const b of map.bindings) for (const k of b.keys) {
      if (shownOnly(k)) continue;
      const t = tokens(k);
      if (t.length < typed.length) continue;
      let arg: string | undefined;
      const fits = typed.every((p, i) => {
        if (t[i] === "{x}") return p.length === 1 && (arg = p) !== undefined;
        return t[i] === p;
      });
      if (!fits) continue;
      if (t.length === typed.length) exact ??= { action: b.action, key: typed.join(""), count, ...(arg !== undefined ? { arg } : {}) };
      else longer = true;
    }
    if (exact && !longer) return this.done(exact);
    if (longer) {
      this.pending = typed;
      this.fallback = exact;
      return { kind: "wait", pending: this.shown };
    }
    // Nothing goes on from here: a waiting shorter binding runs, and this key starts afresh
    if (this.fallback) {
      const f = this.fallback;
      this.reset();
      this.remember(f);
      return { kind: "action", press: f, then: this.feed(map, name, o) };
    }
    const was = this.pending.length ? this.shown : null;
    this.reset();
    return { kind: "none", cancelled: was ? was + name : null };
  }

  private done(press: Press): Fed {
    this.reset();
    this.remember(press);
    return { kind: "action", press };
  }
  private remember(press: Press) {
    if (CHANGES.has(press.action)) this.lastChange = press;
    if (press.action === "find-letter" || press.action === "find-letter-back") this.lastFind = press;
  }
}

/** The action a single key press runs in this keymap (for places without sequences) */
export function actionFor(map: Keymap, e: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): string | null {
  const name = keyName(e);
  return map.bindings.find((b) => b.keys.includes(name))?.action ?? null;
}

/** Keys bound twice in a keymap, or bound and also the start of another of its keys */
export function conflicts(map: Keymap): string[] {
  const seen = new Set<string>();
  const twice = new Set<string>();
  const all = map.bindings.flatMap((b) => b.keys.filter((k) => !shownOnly(k)));
  for (const k of all) (seen.has(k) ? twice : seen).add(k);
  return [...twice];
}

/** Keys breaking the rules in a keymap: "v runs review (the rule: visual)" */
export function ruleBreaks(map: Keymap, rules: Record<string, string> = RULES): string[] {
  return map.bindings.flatMap((b) =>
    b.keys.filter((k) => rules[k] && rules[k] !== b.action && !(map.grid && GRID_MOVES[k] === b.action)).map((k) => `${k} runs ${b.action} (the rule: ${rules[k]})`),
  );
}

const SHOWN: Record<string, string> = { ArrowDown: "↓", ArrowUp: "↑", ArrowLeft: "←", ArrowRight: "→", Escape: "Esc", Backspace: "⌫", "Shift+Tab": "⇧Tab" };

/** How a binding's keys are written: "j/↓", "Esc" */
export const keysLabel = (b: Binding) => b.keys.map((k) => SHOWN[k] ?? k).join(" ");

/** A binding, shorter to write */
export const bind = (keys: string | string[], action: string, does: string): Binding => ({
  keys: typeof keys === "string" ? [keys] : keys,
  action,
  does,
});

/** Bindings most lists share */
export const MOVE = [
  bind(["j", "ArrowDown"], "down", "down"),
  bind(["k", "ArrowUp"], "up", "up"),
  bind("gg", "first", "first"),
  bind("G", "last", "last (5G: the 5th)"),
];
/** Scrolling by pages, the vim way */
export const PAGES = [
  bind(["Ctrl+d", "PageDown"], "page-down", "half a page down"),
  bind("Ctrl+u", "page-up", "half a page up"),
  bind("Ctrl+f", "full-down", "a page down"),
  bind(["Ctrl+b", "PageUp"], "full-up", "a page up"),
];
export const HELP = bind("?", "help", "all keys");

/** How many times a press asks for (its count, or once) */
export const times = (p: Pick<Press, "count">) => Math.max(1, p.count ?? 1);
