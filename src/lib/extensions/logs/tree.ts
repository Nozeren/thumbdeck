// The log as rows on screen: blocks you opened show their contents, hidden levels are left
// out. Pure functions, tested with `node --test` (tree.test.ts).
import type { BlockNode, Line, Node } from "./types.ts";

export type Row =
  | { key: string; depth: number; type: "line"; line: number }
  | { key: string; depth: number; type: "block"; node: BlockNode; open: boolean };

/** A block's identity across reloads (live tail keeps what you opened) */
export const blockKey = (b: BlockNode) => `${b.kind}:${b.first_line}`;

export function rows(outline: Node[], lines: Line[], open: Set<string>, hidden: Set<string>): Row[] {
  const out: Row[] = [];
  const walk = (nodes: Node[], depth: number) => {
    for (const n of nodes) {
      if (n.type === "line") {
        if (!hidden.has(lines[n.line]?.level)) out.push({ key: `line:${n.line}`, depth, type: "line", line: n.line });
      } else {
        const key = blockKey(n);
        const isOpen = open.has(key);
        out.push({ key, depth, type: "block", node: n, open: isOpen });
        if (isOpen) walk(n.children, depth + 1);
      }
    }
  };
  walk(outline, 0);
  return out;
}

/** For each line, the keys of the blocks it's in, outermost first */
export function ancestors(outline: Node[]): Map<number, string[]> {
  const out = new Map<number, string[]>();
  const walk = (nodes: Node[], path: string[]) => {
    for (const n of nodes) {
      if (n.type === "line") out.set(n.line, path);
      else walk(n.children, [...path, blockKey(n)]);
    }
  };
  walk(outline, []);
  return out;
}

/** The parent block row of a row, if any */
export function parentIndex(list: Row[], index: number): number | null {
  const depth = list[index]?.depth ?? 0;
  for (let i = index - 1; i >= 0; i--) if (list[i].depth < depth) return i;
  return null;
}

/** The line a row stands for: a block's first line */
export const lineOf = (r: Row) => (r.type === "line" ? r.line : r.node.first_line);

/**
 * The next (or previous) line after `from` that matches, among lines whose level isn't hidden,
 * wrapping around. null when none does.
 */
export function findLine(lines: Line[], from: number, direction: 1 | -1, hidden: Set<string>,
                         matches: (l: Line) => boolean): number | null {
  const n = lines.length;
  for (let step = 1; step <= n; step++) {
    const i = (((from + direction * step) % n) + n) % n;
    if (!hidden.has(lines[i].level) && matches(lines[i])) return i;
  }
  return null;
}

/** "HH:MM:SS │ LEVEL   │ message" (as qa-log-tui shows a line) */
export function formatLine(l: Line): string {
  const time = /\d{2}:\d{2}:\d{2}/.exec(l.time)?.[0] ?? l.time;
  const parts = [];
  if (time) parts.push(time);
  if (l.level !== "UNKNOWN") parts.push(l.level.padEnd(7));
  parts.push(l.message.replaceAll("\r", "").replaceAll("\n", " "));
  return parts.join(" │ ");
}

/** "5m ago" */
export function age(mtime: number, now = Date.now() / 1000): string {
  const s = Math.max(0, now - mtime);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

// ------------------------------------------------------------ detail view

export interface JsonRow {
  /** Where it is in the entry, e.g. "response.headers" (unique) */
  path: string;
  depth: number;
  key: string;
  value: unknown;
  /** Objects and arrays can be opened */
  container: boolean;
  open: boolean;
  /** "{3}" / "[2]" for containers, the value (shortened) otherwise */
  display: string;
  kind: "string" | "number" | "bool" | "null" | "container";
}

const SHORTEN_AT = 200;

/** One entry as rows; the top-level containers start open (their path is in `closed` when not). */
export function jsonRows(entry: Record<string, unknown>, closed: Set<string>, opened: Set<string>): JsonRow[] {
  const out: JsonRow[] = [];
  const walk = (key: string, value: unknown, path: string, depth: number) => {
    const container = value !== null && typeof value === "object";
    const open = container && (depth === 0 ? !closed.has(path) : opened.has(path));
    let display: string;
    let kind: JsonRow["kind"];
    if (Array.isArray(value)) (display = `[${value.length}]`), (kind = "container");
    else if (container) (display = `{${Object.keys(value as object).length}}`), (kind = "container");
    else if (value === null) (display = "null"), (kind = "null");
    else if (typeof value === "boolean") (display = String(value)), (kind = "bool");
    else if (typeof value === "number") (display = String(value)), (kind = "number");
    else {
      display = JSON.stringify(String(value));
      if (display.length > SHORTEN_AT) display = display.slice(0, SHORTEN_AT) + "…";
      kind = "string";
    }
    out.push({ path, depth, key, value, container, open, display, kind });
    if (open) {
      const entries = Array.isArray(value) ? value.map((v, i) => [`[${i}]`, v] as const) : Object.entries(value as object);
      for (const [k, v] of entries) walk(k, v, `${path}.${k}`, depth + 1);
    }
  };
  for (const [k, v] of Object.entries(entry)) walk(k, v, k, 0);
  return out;
}

/** What `y` copies: a value as text, containers as indented JSON */
export const copyText = (value: unknown) =>
  value !== null && typeof value === "object" ? JSON.stringify(value, null, 2) : String(value);
