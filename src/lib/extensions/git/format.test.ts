import { test } from "node:test";
import assert from "node:assert/strict";
import { groupChanges, lineKind, refNames, track } from "./format.ts";

test("files are grouped: staged, changed (conflicts first), untracked", () => {
  const c = (path: string, staged: string, unstaged: string) => ({ path, from: null, staged, unstaged });
  const rows = groupChanges([c("a", "M", "M"), c("b", " ", "M"), c("u", "U", "U"), c("n", "?", "?"), c("s", "A", " ")]);
  assert.deepEqual(rows.map((r) => `${r.group}:${r.change.path}:${r.state}`), [
    "staged:a:M", "staged:s:A", "changes:u:U", "changes:a:M", "changes:b:M", "untracked:n:?",
  ]);
});

test("diff lines", () => {
  assert.equal(lineKind("diff --git a/x b/x"), "meta");
  assert.equal(lineKind("+++ b/x"), "meta");
  assert.equal(lineKind("@@ -1,2 +1,3 @@ fn main"), "hunk");
  assert.equal(lineKind("+new"), "add");
  assert.equal(lineKind("-old"), "del");
  assert.equal(lineKind(" same"), "text");
  assert.equal(lineKind("A commit message"), "text");
});

test("ahead / behind and ref names", () => {
  assert.equal(track(2, 1), "↑2 ↓1");
  assert.equal(track(0, 0), "");
  assert.deepEqual(refNames("HEAD -> main, origin/main, tag: v1"), ["main", "origin/main", "tag: v1"]);
  assert.deepEqual(refNames(""), []);
});
