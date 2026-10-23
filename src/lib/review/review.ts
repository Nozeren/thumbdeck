// The review page's logic, kept out of the page so it can be tested (review.test.ts): the rows
// of a file (its changes, the unchanged parts between them collapsed or opened), where a
// comment goes on GitHub, the file tree, and comment text made safe to show.
import { splitRows, unifiedRows, type DiffFile, type Line, type Segment } from "./diff.ts";

/** Where a comment sits: GitHub's side and line (LEFT: the old file, RIGHT: the new one) */
export interface Anchor {
  side: "LEFT" | "RIGHT";
  line: number;
}

export type VRow =
  | { kind: "hunk"; header: string; fn: string }
  /** Unchanged lines left out of the diff: `key` opens them (with the file's text) */
  | { kind: "gap"; key: string; count: number; canOpen: boolean }
  /** `extra`: an unchanged line from the file's text, outside the diff (no comments there) */
  | { kind: "line"; left: Line | null; right: Line | null; anchor: Anchor | null; extra: boolean; changeStart: boolean };

export const anchorKey = (a: Anchor) => `${a.side}:${a.line}`;

/** A row's anchor: the new side when the row has it, the old side for a removed line */
export function anchorOf(left: Line | null, right: Line | null): Anchor | null {
  if (right && right.new !== null) return { side: "RIGHT", line: right.new };
  if (left && left.old !== null) return { side: "LEFT", line: left.old };
  return null;
}

/** The lines of a file's text (without the empty one after a last newline) */
export function textLines(text: string): string[] {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

/**
 * The rows the page shows for a file. `text` is the new file's lines when known: the parts
 * between the changes can then be opened (`open(key)` says which are; all of them for the whole
 * file). Without it they stay collapsed.
 */
export function rowsFor(file: DiffFile, split: boolean, text: string[] | null, open: (key: string) => boolean): VRow[] {
  const out: VRow[] = [];
  let newEnd = 0; // the last new line number shown so far
  let oldEnd = 0;
  const unchanged = (from: number, to: number, shift: number, key: string) => {
    const count = to - from + 1;
    if (count <= 0) return;
    if (text && open(key)) {
      for (let n = from; n <= to; n++) {
        const l: Line = { kind: "same", text: text[n - 1] ?? "", old: n + shift, new: n };
        out.push({ kind: "line", left: l, right: l, anchor: null, extra: true, changeStart: false });
      }
    } else out.push({ kind: "gap", key, count, canOpen: !!text });
  };
  file.hunks.forEach((h, i) => {
    const m = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@ ?(.*)$/.exec(h.header);
    const oldStart = m ? Number(m[1]) : 1;
    const newStart = m ? Number(m[2]) : 1;
    // A new file starts at +1 with nothing before; otherwise the lines before this hunk
    unchanged(newEnd + 1, newStart - 1, oldStart - newStart, `gap${i}`);
    out.push({ kind: "hunk", header: h.header, fn: m?.[3] ?? "" });
    const one = { ...file, hunks: [h] };
    for (const r of split ? splitRows(one) : unifiedRows(one)) {
      if (r.header) continue;
      out.push({ kind: "line", left: r.left, right: r.right, anchor: anchorOf(r.left, r.right), extra: false, changeStart: !!r.changeStart });
    }
    for (const l of h.lines) {
      if (l.new !== null) newEnd = Math.max(newEnd, l.new);
      if (l.old !== null) oldEnd = Math.max(oldEnd, l.old);
    }
    // An empty hunk still moves past its start
    newEnd = Math.max(newEnd, newStart - 1);
    oldEnd = Math.max(oldEnd, oldStart - 1);
  });
  if (text && file.hunks.length && file.status !== "deleted") unchanged(newEnd + 1, text.length, oldEnd - newEnd, "end");
  return out;
}

/** A row is a line one can comment on (in the diff, with a place on GitHub) */
export const commentable = (r: VRow | undefined): r is Extract<VRow, { kind: "line" }> & { anchor: Anchor } =>
  !!r && r.kind === "line" && !r.extra && !!r.anchor;

/** A row is a change (an added or removed line, or a pair of them) */
export const isChange = (r: VRow | undefined) =>
  !!r && r.kind === "line" && !r.extra && (r.left?.kind === "del" || r.right?.kind === "add");

/** The comment GitHub gets for the rows from `start` to `end` (one line, or several) */
export function commentRange(start: Anchor, end: Anchor): { line: number; side: Anchor["side"]; startLine?: number; startSide?: Anchor["side"] } {
  if (start.side === end.side && start.line === end.line) return { line: end.line, side: end.side };
  return { line: end.line, side: end.side, startLine: start.line, startSide: start.side };
}

/** "line 12", "lines 12–15", or "3 lines" when they run from the old file into the new one */
export function rangeLabel(lines: Anchor[]): string {
  if (!lines.length) return "";
  const first = lines[0];
  const last = lines[lines.length - 1];
  if (lines.length === 1 || (first.side === last.side && first.line === last.line)) return `line ${last.line}`;
  return lines.every((l) => l.side === first.side) ? `lines ${first.line}–${last.line}` : `${lines.length} lines`;
}

/** The text of the lines that stay (a suggestion replaces those; removed lines can't be) */
export function keptText(rows: Extract<VRow, { kind: "line" }>[]): string[] {
  const kept = rows.map((r) => r.right ?? (r.left?.kind === "same" ? r.left : null)).filter((l): l is Line => !!l);
  return (kept.length ? kept : rows.map((r) => r.left).filter((l): l is Line => !!l)).map((l) => l.text);
}

/** The text a suggestion starts from: the selected lines that stay (removed ones left out) */
export function suggestionText(rows: Extract<VRow, { kind: "line" }>[]): string {
  return "```suggestion\n" + keptText(rows).join("\n") + "\n```";
}

/** Files by folder, in the diff's order within each (for the tree on the left) */
export function fileTree(files: DiffFile[], filter: string): { dir: string; files: number[] }[] {
  const want = filter.trim().toLowerCase();
  const groups: { dir: string; files: number[] }[] = [];
  files.forEach((f, i) => {
    if (want && !f.path.toLowerCase().includes(want)) return;
    const dir = f.path.includes("/") ? f.path.slice(0, f.path.lastIndexOf("/")) : "";
    const g = groups.find((x) => x.dir === dir);
    if (g) g.files.push(i);
    else groups.push({ dir, files: [i] });
  });
  return groups.sort((a, b) => (a.dir === "" ? -1 : b.dir === "" ? 1 : a.dir.localeCompare(b.dir)));
}

const escapeHtml = (s: string) => s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/**
 * A comment's text (Markdown from GitHub, written by anyone) as safe HTML: everything is
 * escaped first, then a little Markdown comes back: code blocks, a suggestion as old / new
 * lines, `code`, **bold**, *italics*, and links as plain text with their address.
 * `original`: the lines a suggestion replaces, to show them.
 */
export function renderBody(text: string, original: string[] = []): string {
  const parts = text.replace(/\r\n/g, "\n").split(/^```/m);
  let html = "";
  parts.forEach((part, i) => {
    if (i % 2 === 1) {
      const nl = part.indexOf("\n");
      const lang = nl < 0 ? part.trim() : part.slice(0, nl).trim();
      const body = (nl < 0 ? "" : part.slice(nl + 1)).replace(/\n$/, "");
      if (lang === "suggestion") {
        const old = original.map((l) => `<div class="sg-old">− ${escapeHtml(l)}</div>`).join("");
        const neu = body.split("\n").map((l) => `<div class="sg-new">+ ${escapeHtml(l)}</div>`).join("");
        html += `<div class="suggestion"><div class="sg-head">Suggested change</div>${old}${neu}</div>`;
      } else html += `<pre class="code">${escapeHtml(body)}</pre>`;
      return;
    }
    const inline = escapeHtml(part.trim())
      .replace(/`([^`\n]+)`/g, "<code>$1</code>")
      .replace(/\*\*([^*\n]+)\*\*/g, "<b>$1</b>")
      .replace(/(^|[\s(])\*([^*\n]+)\*(?=[\s).,!?]|$)/g, "$1<i>$2</i>")
      .replace(/\[([^\]\n]+)\]\((https?:[^)\s]+)\)/g, "$1 ($2)");
    if (inline) html += inline.split(/\n{2,}/).map((p) => `<p>${p.replace(/\n/g, "<br>")}</p>`).join("");
  });
  return html;
}

/** What a search looks for: "+word" only in added lines, "-word" only in removed ones ("\-word"
 *  for a leading - or +); a capital letter makes it match case, as vim's smartcase does */
export interface Query {
  text: string;
  only: "add" | "del" | null;
  matchCase: boolean;
}

export function parseQuery(input: string): Query | null {
  let text = input;
  let only: Query["only"] = null;
  if (text.startsWith("+")) (only = "add"), (text = text.slice(1));
  else if (text.startsWith("-")) (only = "del"), (text = text.slice(1));
  else if (text.startsWith("\\")) text = text.slice(1);
  if (!text) return null;
  return { text, only, matchCase: text !== text.toLowerCase() };
}

/** Where a query is in a line's text: [start, end) ranges */
export function matchRanges(text: string, q: Query): [number, number][] {
  const hay = q.matchCase ? text : text.toLowerCase();
  const needle = q.matchCase ? q.text : q.text.toLowerCase();
  const out: [number, number][] = [];
  for (let i = hay.indexOf(needle); i >= 0; i = hay.indexOf(needle, i + needle.length)) out.push([i, i + needle.length]);
  return out;
}

/** Every line of the changes that matches, file by file, in order */
export function findMatches(files: DiffFile[], q: Query): { file: number; line: Line }[] {
  const out: { file: number; line: Line }[] = [];
  files.forEach((f, file) => {
    for (const h of f.hunks) for (const line of h.lines) if ((!q.only || line.kind === q.only) && matchRanges(line.text, q).length) out.push({ file, line });
  });
  return out;
}

/** A line cut where the changed words and the search matches start and end, to mark each piece */
export function markPieces(text: string, words: Segment[] | null, hits: [number, number][]): { text: string; changed: boolean; hit: boolean }[] {
  const changedAt: boolean[] = [];
  if (words) for (const s of words) for (let k = 0; k < s.text.length; k++) changedAt.push(s.changed);
  const hitAt = new Array<boolean>(text.length).fill(false);
  for (const [a, b] of hits) for (let k = a; k < b; k++) hitAt[k] = true;
  const out: { text: string; changed: boolean; hit: boolean }[] = [];
  for (let k = 0; k < text.length; k++) {
    const changed = !!changedAt[k];
    const last = out.at(-1);
    if (last && last.changed === changed && last.hit === hitAt[k]) last.text += text[k];
    else out.push({ text: text[k], changed, hit: hitAt[k] });
  }
  return out;
}
