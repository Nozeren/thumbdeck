import { test } from "node:test";
import assert from "node:assert/strict";
import { checksText, isStale, reviewText, splitList } from "./format.ts";

test("stale after the set number of days", () => {
  const now = Date.parse("2026-09-26T12:00:00Z");
  assert.equal(isStale("2026-09-19T12:00:00Z", 7, now), true);
  assert.equal(isStale("2026-09-20T12:00:00Z", 7, now), false);
  assert.equal(isStale("2026-01-01T00:00:00Z", 0, now), false, "0: never stale");
  assert.equal(isStale("", 7, now), false);
});

test("states in words", () => {
  assert.equal(checksText("FAILURE"), "✘ failed");
  assert.equal(checksText(""), "no checks");
  assert.equal(reviewText("CHANGES_REQUESTED"), "✘ changes requested");
  assert.equal(reviewText("NEW_STATE"), "new_state");
});

test("comma lists", () => {
  assert.deepEqual(splitList(" a, b ,,c "), ["a", "b", "c"]);
  assert.deepEqual(splitList(""), []);
});
