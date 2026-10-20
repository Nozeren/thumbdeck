import { test } from "node:test";
import assert from "node:assert/strict";
import { bind, conflicts, keyName, KeyReader, RESERVED, ruleBreaks, tokens, type Fed, type Keymap } from "./keys.ts";
import { ALL } from "./maps.ts";

const press = (key: string, mods: Partial<KeyboardEvent> = {}) => ({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...mods });

test("no key means two things in one place", () => {
  for (const map of ALL) assert.deepEqual(conflicts(map), [], map.name);
});

test("a key means the same everywhere it's used", () => {
  for (const map of ALL) assert.deepEqual(ruleBreaks(map), [], map.name);
});

test("thumbdeck's own keys stay its own", () => {
  // the app's places may use the reserved keys; this only checks the list is well formed
  assert.ok(RESERVED.includes("Space Space") && RESERVED.includes("gt") && RESERVED.includes("5"));
});

test("key names", () => {
  assert.equal(keyName(press("j")), "j");
  assert.equal(keyName(press("G", { shiftKey: true })), "G");
  assert.equal(keyName(press("?", { shiftKey: true })), "?");
  assert.equal(keyName(press(" ")), "Space");
  assert.equal(keyName(press("Tab", { shiftKey: true })), "Shift+Tab");
  assert.equal(keyName(press("d", { ctrlKey: true })), "Ctrl+d");
});

test("keys as presses", () => {
  assert.deepEqual(tokens("gg"), ["g", "g"]);
  assert.deepEqual(tokens("]c"), ["]", "c"]);
  assert.deepEqual(tokens("Ctrl+w o"), ["Ctrl+w", "o"]);
  assert.deepEqual(tokens("Space y j"), ["Space", "y", "j"]);
  assert.deepEqual(tokens("f{x}"), ["f", "{x}"]);
  assert.deepEqual(tokens("'{x}"), ["'", "{x}"]);
  assert.deepEqual(tokens("Enter"), ["Enter"]);
  assert.deepEqual(tokens("Shift+Tab"), ["Shift+Tab"]);
  assert.deepEqual(tokens("Ctrl+d"), ["Ctrl+d"]);
});

const MAP: Keymap = {
  name: "T",
  bindings: [
    bind("j", "down", ""), bind("gg", "first", ""), bind("G", "last", ""), bind("dd", "delete", ""), bind("x", "tick", ""),
    bind("yy", "yank", ""), bind("f{x}", "find-letter", ""), bind("F{x}", "find-letter-back", ""), bind("]c", "next-change", ""),
    bind("Space s", "stop", ""), bind("Space y j", "copy-json", ""), bind("Space y p", "copy-path", ""), bind("Ctrl+w o", "wide", ""),
    bind("z", "zed", ""), bind("zz", "center", ""), bind("1–9", "shown", ""),
  ],
};
function typing(keys: string[], o = {}) {
  const r = new KeyReader();
  return { r, out: keys.map((k) => r.feed(MAP, k, o)) };
}
const act = (f: Fed): string => (f.kind === "action" ? `${f.press.action}${f.press.count !== null ? `×${f.press.count}` : ""}${f.press.arg ? `(${f.press.arg})` : ""}${f.then ? `+${act(f.then)}` : ""}` : f.kind);

test("sequences, counts and letters", () => {
  assert.deepEqual(typing(["j"]).out.map(act), ["down"]);
  assert.deepEqual(typing(["g", "g"]).out.map(act), ["wait", "first"]);
  assert.deepEqual(typing(["5", "j"]).out.map(act), ["wait", "down×5"]);
  assert.deepEqual(typing(["1", "0", "G"]).out.map(act), ["wait", "wait", "last×10"]);
  assert.deepEqual(typing(["3", "d", "d"]).out.map(act), ["wait", "wait", "delete×3"]);
  assert.deepEqual(typing(["f", "p"]).out.map(act), ["wait", "find-letter(p)"]);
  assert.deepEqual(typing(["]", "c"]).out.map(act), ["wait", "next-change"]);
  assert.deepEqual(typing(["Space", "y", "p"]).out.map(act), ["wait", "wait", "copy-path"]);
  assert.deepEqual(typing(["Ctrl+w", "o"]).out.map(act), ["wait", "wide"]);
  assert.deepEqual(typing(["5", "j"], { counts: false }).out.map(act), ["none", "down"], "no counts: digits aren't taken");
});

test("what's typed so far shows", () => {
  const r = new KeyReader();
  r.feed(MAP, "3");
  r.feed(MAP, "d");
  assert.equal(r.shown, "3d");
  r.reset();
  r.feed(MAP, "Space");
  assert.equal(r.shown, "Space ");
  assert.deepEqual(r.next(MAP).map(([k]) => k).sort(), ["s", "y j", "y p"]);
});

test("a wrong key or Esc cancels", () => {
  const { out } = typing(["d", "j"]);
  assert.deepEqual(out.map((f) => (f.kind === "none" ? `none:${f.cancelled}` : act(f))), ["wait", "none:dj"]);
  assert.deepEqual(typing(["g", "Escape", "j"]).out.map(act), ["wait", "none", "down"]);
  assert.deepEqual(typing(["q"]).out.map((f) => (f.kind === "none" ? `none:${f.cancelled}` : act(f))), ["none:null"]);
});

test("a shorter key runs when the next key doesn't go on", () => {
  // z and zz: z waits; j after it runs z, then j
  assert.deepEqual(typing(["z", "z"]).out.map(act), ["wait", "center"]);
  assert.deepEqual(typing(["z", "j"]).out.map(act), ["wait", "zed+down"]);
});

test(". repeats the last change, ; and , the last f", () => {
  assert.deepEqual(typing(["d", "d", "j", "."]).out.map(act), ["wait", "delete", "down", "delete"]);
  assert.deepEqual(typing(["x", "3", "."]).out.map(act), ["tick", "wait", "tick×3"]);
  assert.deepEqual(typing(["f", "a", ";", ","]).out.map(act), ["wait", "find-letter(a)", "find-letter(a)", "find-letter-back(a)"]);
  assert.deepEqual(typing(["y", "y", "."]).out.map(act), ["wait", "yank", "none"], "copying isn't a change");
});

test("h / l move sideways only in a grid", () => {
  const kit = { name: "KIT", grid: true, bindings: [bind("l", "right", "right")] };
  assert.deepEqual(ruleBreaks(kit), []);
  assert.deepEqual(ruleBreaks({ ...kit, grid: false }), ["l runs right (the rule: open)"]);
});
