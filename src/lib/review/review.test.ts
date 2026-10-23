import { test } from "node:test";
import assert from "node:assert/strict";
import { parseDiff } from "./diff.ts";
import { anchorOf, commentRange, fileTree, findMatches, markPieces, matchRanges, parseQuery, isChange, rangeLabel, renderBody, rowsFor, suggestionText, textLines, type VRow } from "./review.ts";

const DIFF = `diff --git a/src/cart.ts b/src/cart.ts
--- a/src/cart.ts
+++ b/src/cart.ts
@@ -3,4 +3,5 @@ import x
 const TAX = 0.21;
-export const a = 1;
+export const a = 2;
+export const b = 3;
 // end of block
@@ -20,2 +21,2 @@ export function total() {
-  return 1;
+  return 2;
 }
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,1 +1,2 @@
 # shop
+More.
`;
const TEXT = Array.from({ length: 25 }, (_, i) => `line ${i + 1}`);

const lines = (rows: VRow[]) => rows.filter((r) => r.kind === "line") as Extract<VRow, { kind: "line" }>[];

test("collapsed parts between changes, and before and after them", () => {
  const [f] = parseDiff(DIFF);
  const rows = rowsFor(f, false, TEXT, () => false);
  assert.deepEqual(rows.filter((r) => r.kind === "gap").map((r) => r.kind === "gap" && [r.key, r.count]), [["gap0", 2], ["gap1", 14], ["end", 3]]);
  // Without the file's text: only the parts between hunks, and they can't open
  const bare = rowsFor(f, false, null, () => true);
  assert.deepEqual(bare.filter((r) => r.kind === "gap").map((r) => r.kind === "gap" && [r.count, r.canOpen]), [[2, false], [14, false]]);
});

test("the whole file: every line once, numbered on both sides, nothing to comment on outside the diff", () => {
  const [f] = parseDiff(DIFF);
  const all = lines(rowsFor(f, false, TEXT, () => true));
  const news = all.map((r) => (r.right ?? r.left)!.new).filter((n) => n !== null);
  assert.deepEqual(news, Array.from({ length: 25 }, (_, i) => i + 1));
  const extra = all.find((r) => r.extra && r.right?.new === 10)!;
  assert.equal(extra.left!.old, 9, "between the hunks the old file is one line behind");
  assert.equal(extra.anchor, null);
  assert.equal(extra.right!.text, "line 10");
});

test("anchors: the new side, or the old one for a removed line; ranges for GitHub", () => {
  const [f] = parseDiff(DIFF);
  const rows = lines(rowsFor(f, false, null, () => false));
  assert.deepEqual(rows[1].anchor, { side: "LEFT", line: 4 });
  assert.deepEqual(rows[2].anchor, { side: "RIGHT", line: 4 });
  assert.deepEqual(anchorOf(null, null), null);
  assert.deepEqual(commentRange({ side: "RIGHT", line: 4 }, { side: "RIGHT", line: 4 }), { line: 4, side: "RIGHT" });
  assert.deepEqual(commentRange({ side: "LEFT", line: 4 }, { side: "RIGHT", line: 5 }), { line: 5, side: "RIGHT", startLine: 4, startSide: "LEFT" });
  assert.equal(rangeLabel([{ side: "RIGHT", line: 4 }, { side: "RIGHT", line: 5 }]), "lines 4–5");
  assert.equal(rangeLabel([{ side: "RIGHT", line: 4 }]), "line 4");
  assert.equal(rangeLabel([{ side: "LEFT", line: 8 }, { side: "LEFT", line: 9 }, { side: "RIGHT", line: 7 }]), "3 lines", "old and new numbers don't make a range");
});

test("side by side pairs a removed line with the one replacing it", () => {
  const [f] = parseDiff(DIFF);
  const rows = lines(rowsFor(f, true, null, () => false));
  const pair = rows.find((r) => r.left?.kind === "del" && r.right?.kind === "add")!;
  assert.equal(pair.left!.text, "export const a = 1;");
  assert.equal(pair.right!.text, "export const a = 2;");
  assert.ok(isChange(pair));
  assert.ok(!isChange(rows[0]));
});

test("a suggestion starts from the selected lines that stay", () => {
  const [f] = parseDiff(DIFF);
  const rows = lines(rowsFor(f, false, null, () => false)).slice(0, 4);
  assert.equal(suggestionText(rows), "```suggestion\nconst TAX = 0.21;\nexport const a = 2;\nexport const b = 3;\n```");
});

test("the file tree groups by folder, files at the top first, and filters", () => {
  const files = parseDiff(DIFF);
  assert.deepEqual(fileTree(files, ""), [{ dir: "", files: [1] }, { dir: "src", files: [0] }]);
  assert.deepEqual(fileTree(files, "CART"), [{ dir: "src", files: [0] }]);
});

test("comment text is escaped, with a little Markdown back", () => {
  const html = renderBody('<img src=x onerror="alert(1)"> use `total()` **now**\n\n```suggestion\nreturn 2;\n```', ["return 1;"]);
  assert.ok(!html.includes("<img"), "no HTML from the comment");
  assert.ok(html.includes("&lt;img src=x onerror=&quot;alert(1)&quot;&gt;"));
  assert.ok(html.includes("<code>total()</code>") && html.includes("<b>now</b>"));
  assert.ok(html.includes('<div class="sg-old">− return 1;</div>') && html.includes('<div class="sg-new">+ return 2;</div>'));
  assert.equal(renderBody("[docs](javascript:alert(1))"), "<p>[docs](javascript:alert(1))</p>", "only http(s) links become text");
});

test("a file's lines", () => {
  assert.deepEqual(textLines("a\nb\n"), ["a", "b"]);
  assert.deepEqual(textLines("a\r\nb"), ["a", "b"]);
});

test("search: what the query asks", () => {
  assert.deepEqual(parseQuery("total"), { text: "total", only: null, matchCase: false });
  assert.deepEqual(parseQuery("+return"), { text: "return", only: "add", matchCase: false });
  assert.deepEqual(parseQuery("-TAX"), { text: "TAX", only: "del", matchCase: true });
  assert.deepEqual(parseQuery("\\-1"), { text: "-1", only: null, matchCase: false });
  assert.equal(parseQuery(""), null);
  assert.equal(parseQuery("+"), null);
});

test("search: where it matches", () => {
  assert.deepEqual(matchRanges("a Total total", parseQuery("total")!), [[2, 7], [8, 13]]);
  assert.deepEqual(matchRanges("a Total total", parseQuery("Total")!), [[2, 7]]);
  assert.deepEqual(matchRanges("aaaa", parseQuery("aa")!), [[0, 2], [2, 4]]);
  const files = parseDiff(DIFF);
  const all = findMatches(files, parseQuery("return")!);
  assert.deepEqual(all.map((m) => [m.file, m.line.kind, m.line.text]), [[0, "del", "  return 1;"], [0, "add", "  return 2;"]]);
  assert.deepEqual(findMatches(files, parseQuery("+return")!).map((m) => m.line.text), ["  return 2;"]);
  assert.deepEqual(findMatches(files, parseQuery("nothing like it")!), []);
});

test("search: a line's pieces, the changed words and the matches marked", () => {
  const words = [{ text: "export const a = ", changed: false }, { text: "2", changed: true }, { text: ";", changed: false }];
  assert.deepEqual(markPieces("export const a = 2;", words, [[7, 12]]), [
    { text: "export ", changed: false, hit: false },
    { text: "const", changed: false, hit: true },
    { text: " a = ", changed: false, hit: false },
    { text: "2", changed: true, hit: false },
    { text: ";", changed: false, hit: false },
  ]);
  assert.deepEqual(markPieces("ab", null, []), [{ text: "ab", changed: false, hit: false }]);
});
