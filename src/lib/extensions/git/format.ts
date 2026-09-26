// Small pure helpers for the Git tab, tested with `node --test` (format.test.ts).
import type { Change } from "./types.ts";

export type Group = "staged" | "changes" | "untracked";

/** A row of the Changes list: a file in one group (a file can be both staged and changed) */
export interface FileRow {
  group: Group;
  change: Change;
  /** Its state in this group */
  state: string;
}

/** Staged, then changed (conflicts first), then untracked */
export function groupChanges(changes: Change[]): FileRow[] {
  const staged = changes.filter((c) => c.staged !== " " && c.staged !== "?" && c.staged !== "U");
  const changed = changes.filter((c) => c.unstaged !== " " && c.unstaged !== "?").sort((a, b) => Number(b.unstaged === "U") - Number(a.unstaged === "U"));
  const untracked = changes.filter((c) => c.unstaged === "?");
  return [
    ...staged.map((change) => ({ group: "staged" as const, change, state: change.staged })),
    ...changed.map((change) => ({ group: "changes" as const, change, state: change.unstaged })),
    ...untracked.map((change) => ({ group: "untracked" as const, change, state: "?" })),
  ];
}

export const stateWord: Record<string, string> = {
  M: "modified", A: "added", D: "deleted", R: "renamed", C: "copied", U: "conflict", T: "type changed", "?": "new",
};

export type LineKind = "add" | "del" | "hunk" | "meta" | "text";

/** How to colour a line of a diff (or of `git show`: the message lines are plain text) */
export function lineKind(line: string): LineKind {
  if (/^(diff |index |--- |\+\+\+ |new file|deleted file|similarity|rename |old mode|new mode|Binary files)/.test(line)) return "meta";
  if (line.startsWith("@@")) return "hunk";
  if (line.startsWith("+")) return "add";
  if (line.startsWith("-")) return "del";
  return "text";
}

/** "↑2 ↓1", "" when even */
export function track(ahead: number, behind: number): string {
  return [ahead ? `↑${ahead}` : "", behind ? `↓${behind}` : ""].filter(Boolean).join(" ");
}

/** Branch and tag names from git's %D ("HEAD -> main, origin/main, tag: v1") */
export function refNames(refs: string): string[] {
  return refs.split(", ").map((r) => r.replace(/^HEAD -> /, "")).filter((r) => r && r !== "HEAD");
}
