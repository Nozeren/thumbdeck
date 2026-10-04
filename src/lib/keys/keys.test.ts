import { test } from "node:test";
import assert from "node:assert/strict";
import { actionFor, bind, conflicts, keyName, ruleBreaks } from "./keys.ts";
import { ALL, REVIEW, REVIEW_FILES } from "./maps.ts";

const press = (key: string, mods: Partial<KeyboardEvent> = {}) => ({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...mods });

test("no key means two things in one place", () => {
  for (const map of ALL) assert.deepEqual(conflicts(map), [], map.name);
});

test("a key means the same everywhere it's used", () => {
  for (const map of ALL) assert.deepEqual(ruleBreaks(map), [], map.name);
});

test("key names", () => {
  assert.equal(keyName(press("j")), "j");
  assert.equal(keyName(press("G", { shiftKey: true })), "G");
  assert.equal(keyName(press("?", { shiftKey: true })), "?");
  assert.equal(keyName(press(" ")), "Space");
  assert.equal(keyName(press("Tab", { shiftKey: true })), "Shift+Tab");
  assert.equal(keyName(press("d", { ctrlKey: true })), "Ctrl+d");
});

test("looking keys up", () => {
  assert.equal(actionFor(REVIEW, press("n")), "change");
  assert.equal(actionFor(REVIEW_FILES, press("Escape")), "leave");
  assert.equal(actionFor(REVIEW, press("d", { ctrlKey: true })), null, "Ctrl+d isn't d");
  assert.equal(actionFor(REVIEW, press("Escape")), "leave");
});

test("h / l move sideways only in a grid", () => {
  const kit = { name: "KIT", grid: true, bindings: [bind("l", "right", "right")] };
  assert.deepEqual(ruleBreaks(kit), []);
  assert.deepEqual(ruleBreaks({ ...kit, grid: false }), ["l runs right (the rule: open)"]);
});
