// Keys, the same way everywhere: each place (the main page, a tab in one of its modes, the
// review page) lists its keys once, as a Keymap. Its key handler looks keys up there, and the
// ? help is drawn from it, so they can't drift apart. RULES says what a key
// means wherever it's used (keys.test.ts checks every keymap against it).

export interface Binding {
  /** Key names (see keyName): "j", "Enter", "Space", "Shift+Tab", "Ctrl+d" */
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
  /** A grid of buttons (the Toolkit): h / l move left / right there, not back / open */
  grid?: boolean;
}

/** The same key, the same meaning: key -> the action it must run wherever it's bound */
export const RULES: Record<string, string> = {
  j: "down", k: "up", g: "first", G: "last",
  d: "page-down", u: "page-up",
  l: "open",
  Tab: "next-list", "Shift+Tab": "previous-list",
  v: "review", o: "outside", r: "refresh",
  q: "leave", Escape: "leave",
  S: "setup", "?": "help",
};

/** What h / l do in a grid instead of their rule */
const GRID_MOVES: Record<string, string> = { h: "left", l: "right" };

/** A key press as a name: "j", "G", "Enter", "Space", "Shift+Tab", "Ctrl+d" */
export function keyName(e: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): string {
  const key = e.key === " " ? "Space" : e.key;
  // Shift is part of letters and symbols already ("G", "?"); only named keys spell it out
  const shift = e.shiftKey && key.length > 1 && key !== "Space" ? "Shift+" : "";
  const mods = `${e.ctrlKey ? "Ctrl+" : ""}${e.altKey ? "Alt+" : ""}${e.metaKey ? "Meta+" : ""}`;
  return mods + shift + key;
}

/** The action a key press runs in this keymap, if it's one of its keys */
export function actionFor(map: Keymap, e: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): string | null {
  const name = keyName(e);
  return map.bindings.find((b) => b.keys.includes(name))?.action ?? null;
}

/** Keys bound twice in a keymap */
export function conflicts(map: Keymap): string[] {
  const seen = new Set<string>();
  const twice = new Set<string>();
  for (const b of map.bindings) for (const k of b.keys) (seen.has(k) ? twice : seen).add(k);
  return [...twice];
}

/** Keys breaking RULES in a keymap: "v runs open (the rule: review)" */
export function ruleBreaks(map: Keymap): string[] {
  return map.bindings.flatMap((b) =>
    b.keys.filter((k) => RULES[k] && RULES[k] !== b.action && !(map.grid && GRID_MOVES[k] === b.action)).map((k) => `${k} runs ${b.action} (the rule: ${RULES[k]})`),
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
  bind("g", "first", "first"),
  bind("G", "last", "last"),
];
export const HELP = bind("?", "help", "all keys");
