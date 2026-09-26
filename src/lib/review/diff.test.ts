import { test } from "node:test";
import assert from "node:assert/strict";
import { parseDiff, splitRows, unifiedRows, wordDiff } from "./diff.ts";

const DIFF = `commit message line (git show) is skipped
diff --git a/src/app.py b/src/app.py
index 111..222 100644
--- a/src/app.py
+++ b/src/app.py
@@ -1,4 +1,5 @@ def main():
 import os
-x = 1
-y = 2
+x = 10
+y = 2
+z = 3
 print(x)
\\ No newline at end of file
diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..333
--- /dev/null
+++ b/new.txt
@@ -0,0 +1 @@
+hello
diff --git a/old.txt b/old.txt
deleted file mode 100644
--- a/old.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
diff --git a/a.rs b/b.rs
similarity index 90%
rename from a.rs
rename to b.rs
diff --git a/logo.png b/logo.png
Binary files a/logo.png and b/logo.png differ
`;

test("files, their kind and counts", () => {
  const files = parseDiff(DIFF);
  assert.deepEqual(files.map((f) => [f.path, f.status, f.additions, f.deletions]), [
    ["src/app.py", "modified", 3, 2],
    ["new.txt", "added", 1, 0],
    ["old.txt", "deleted", 0, 1],
    ["b.rs", "renamed", 0, 0],
    ["logo.png", "modified", 0, 0],
  ]);
  assert.equal(files[3].oldPath, "a.rs");
  assert.equal(files[4].binary, true);
  const lines = files[0].hunks[0].lines;
  assert.deepEqual(lines.map((l) => [l.kind, l.old, l.new]), [
    ["same", 1, 1], ["del", 2, null], ["del", 3, null], ["add", null, 2], ["add", null, 3], ["add", null, 4], ["same", 4, 5],
  ]);
  assert.equal(files[0].hunks[0].header, "@@ -1,4 +1,5 @@ def main():");
});

test("fingerprints change with the changes", () => {
  const a = parseDiff(DIFF)[0].hash;
  assert.equal(parseDiff(DIFF)[0].hash, a);
  assert.notEqual(parseDiff(DIFF.replace("+z = 3", "+z = 4"))[0].hash, a);
});

test("side by side pairs removed lines with their replacements", () => {
  const rows = splitRows(parseDiff(DIFF)[0]);
  const show = rows.map((r) => r.header ? "@@" : `${r.left?.text ?? "·"} | ${r.right?.text ?? "·"}${r.changeStart ? " *" : ""}`);
  assert.deepEqual(show, ["@@", "import os | import os", "x = 1 | x = 10 *", "y = 2 | y = 2", "· | z = 3", "print(x) | print(x)"]);
});

test("one column keeps every line once", () => {
  const rows = unifiedRows(parseDiff(DIFF)[0]);
  assert.equal(rows.length, 8);
  assert.equal(rows.filter((r) => r.changeStart).length, 1);
});

test("changed words", () => {
  const d = wordDiff("let total = price * 2;", "let total = price * 3;")!;
  assert.deepEqual(d.before.filter((s) => s.changed).map((s) => s.text), ["2"]);
  assert.deepEqual(d.after.filter((s) => s.changed).map((s) => s.text), ["3"]);
  assert.equal(d.after.map((s) => s.text).join(""), "let total = price * 3;");
  assert.equal(wordDiff("completely different", "nothing alike here at all"), null, "rewritten lines get no word marks");
});

test("the Pull requests demo's diff", async () => {
  const { readFileSync } = await import("node:fs");
  const files = parseDiff(readFileSync(new URL("../../../src-tauri/src/extensions/prs/testdata/demo.diff", import.meta.url), "utf8"));
  assert.deepEqual(files.map((f) => [f.path, f.status]), [["src/checkout.py", "modified"], ["src/discounts.py", "added"], ["features/checkout.feature", "modified"]]);
});
