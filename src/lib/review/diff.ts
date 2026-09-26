// Reading a unified diff (git diff, git show, gh pr diff) into files, and a file's changes
// into rows for the review page: side by side (removed lines next to the lines that replaced
// them) or in one column, with the words that changed within a line. Pure, tested with
// `node --test` (diff.test.ts).

export type Kind = "same" | "del" | "add";

export interface Line {
  kind: Kind;
  text: string;
  /** Line number in the old and new file (null where it doesn't exist) */
  old: number | null;
  new: number | null;
}

export interface Hunk {
  /** The "@@ -1,4 +1,5 @@ fn main" line */
  header: string;
  lines: Line[];
}

export interface DiffFile {
  /** The new path (the old one for a deleted file) */
  path: string;
  /** Where a renamed file came from */
  oldPath: string | null;
  status: "modified" | "added" | "deleted" | "renamed";
  binary: boolean;
  hunks: Hunk[];
  additions: number;
  deletions: number;
  /** A fingerprint of its changes: "viewed" is forgotten when they change */
  hash: string;
}

const unquote = (p: string) => (p.startsWith('"') ? p.slice(1, -1).replace(/\\(.)/g, "$1") : p);
const stripPrefix = (p: string) => unquote(p).replace(/^[ab]\//, "");

/** A short fingerprint of a text (FNV-1a) */
export function fingerprint(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 0x01000193);
  return (h >>> 0).toString(36);
}

/** The files of a unified diff; anything before the first file (a commit message) is skipped */
export function parseDiff(text: string): DiffFile[] {
  const files: DiffFile[] = [];
  let file: DiffFile | null = null;
  let hunk: Hunk | null = null;
  let body: string[] = [];
  let oldN = 0;
  let newN = 0;

  const finish = () => {
    if (file) file.hash = fingerprint(body.join("\n"));
    body = [];
  };

  for (const raw of text.split("\n")) {
    const line = raw.replace(/\r$/, "");
    if (line.startsWith("diff --git ")) {
      finish();
      // "diff --git a/x b/x": the paths are refined by the lines that follow
      const m = /^diff --git (".*?"|\S+) (".*?"|\S+)$/.exec(line);
      const path = m ? stripPrefix(m[2]) : line.slice(11);
      file = { path, oldPath: null, status: "modified", binary: false, hunks: [], additions: 0, deletions: 0, hash: "" };
      files.push(file);
      hunk = null;
      continue;
    }
    if (!file) continue;
    if (!hunk || !/^[ +\-\\]/.test(line) || line.startsWith("--- ") || line.startsWith("+++ ")) {
      // Header lines of the file
      if (line.startsWith("new file")) file.status = "added";
      else if (line.startsWith("deleted file")) file.status = "deleted";
      else if (line.startsWith("rename from ")) (file.oldPath = line.slice(12)), (file.status = "renamed");
      else if (line.startsWith("rename to ")) file.path = line.slice(10);
      else if (line.startsWith("Binary files ") || line === "GIT binary patch") file.binary = true;
      else if (line.startsWith("--- ") && line !== "--- /dev/null" && file.status === "deleted") file.path = stripPrefix(line.slice(4));
      else if (line.startsWith("+++ ") && line !== "+++ /dev/null") file.path = stripPrefix(line.slice(4));
      else if (line.startsWith("@@")) {
        const m = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
        oldN = m ? Number(m[1]) : 1;
        newN = m ? Number(m[2]) : 1;
        hunk = { header: line, lines: [] };
        file.hunks.push(hunk);
        body.push(line);
      }
      continue;
    }
    body.push(line);
    const text = line.slice(1);
    if (line.startsWith("+")) {
      hunk.lines.push({ kind: "add", text, old: null, new: newN++ });
      file.additions++;
    } else if (line.startsWith("-")) {
      hunk.lines.push({ kind: "del", text, old: oldN++, new: null });
      file.deletions++;
    } else if (line.startsWith(" ") || line === "") {
      hunk.lines.push({ kind: "same", text, old: oldN++, new: newN++ });
    }
    // "\ No newline at end of file" is left out
  }
  finish();
  return files;
}

export interface Row {
  /** A hunk's header row, or a row of lines */
  header?: string;
  left: Line | null;
  right: Line | null;
  /** The first row of a block of changes (for "next change") */
  changeStart?: boolean;
}

/** Side by side: unchanged lines on both sides; within a block of changes, removed lines on
 *  the left next to the added lines that replaced them */
export function splitRows(file: DiffFile): Row[] {
  const rows: Row[] = [];
  for (const h of file.hunks) {
    rows.push({ header: h.header, left: null, right: null });
    let i = 0;
    while (i < h.lines.length) {
      const l = h.lines[i];
      if (l.kind === "same") {
        rows.push({ left: l, right: l });
        i++;
        continue;
      }
      const dels: Line[] = [];
      const adds: Line[] = [];
      while (i < h.lines.length && h.lines[i].kind === "del") dels.push(h.lines[i++]);
      while (i < h.lines.length && h.lines[i].kind === "add") adds.push(h.lines[i++]);
      for (let k = 0; k < Math.max(dels.length, adds.length); k++) {
        rows.push({ left: dels[k] ?? null, right: adds[k] ?? null, changeStart: k === 0 });
      }
    }
  }
  return rows;
}

/** One column: every line once, in order (a line sits on the side its kind belongs to) */
export function unifiedRows(file: DiffFile): Row[] {
  const rows: Row[] = [];
  for (const h of file.hunks) {
    rows.push({ header: h.header, left: null, right: null });
    h.lines.forEach((l, i) => {
      const prev = h.lines[i - 1];
      rows.push({ left: l.kind === "add" ? null : l, right: l.kind === "del" ? null : l, changeStart: l.kind !== "same" && (!prev || prev.kind === "same") });
    });
  }
  return rows;
}

export interface Segment {
  text: string;
  changed: boolean;
}

/** Words, runs of spaces, and single other characters */
const tokens = (s: string) => s.match(/\w+|\s+|[^\w\s]/g) ?? [];

/** The words that changed between a removed line and the line that replaced it. Lines too
 *  different (or too long) get no word marks: the whole line is the change. */
export function wordDiff(before: string, after: string): { before: Segment[]; after: Segment[] } | null {
  const a = tokens(before);
  const b = tokens(after);
  if (a.length * b.length > 250_000) return null;
  // Longest common subsequence of tokens
  const dp = Array.from({ length: a.length + 1 }, () => new Uint16Array(b.length + 1));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  }
  const common = dp[0][0];
  const sameChars = (() => {
    let n = 0;
    for (let i = 0, j = 0; i < a.length && j < b.length; ) {
      if (a[i] === b[j]) (n += a[i].length), i++, j++;
      else if (dp[i + 1][j] >= dp[i][j + 1]) i++;
      else j++;
    }
    return n;
  })();
  // Mostly rewritten: marking words would be noise
  if (!common || sameChars < 0.4 * Math.max(before.length, after.length)) return null;

  const out = { before: [] as Segment[], after: [] as Segment[] };
  const push = (side: Segment[], text: string, changed: boolean) => {
    const last = side.at(-1);
    if (last && last.changed === changed) last.text += text;
    else side.push({ text, changed });
  };
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      push(out.before, a[i++], false);
      push(out.after, b[j++], false);
    } else if (j >= b.length || (i < a.length && dp[i + 1][j] >= dp[i][j + 1])) push(out.before, a[i++], true);
    else push(out.after, b[j++], true);
  }
  return out;
}
