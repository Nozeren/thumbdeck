import { test } from "node:test";
import assert from "node:assert/strict";
import { combine, NO_SIGNALS, pickMood, RECENT_MS, SLEEP_MS, type PluginSignals, type Signals } from "./mood.ts";
import { CHARACTERS, frame, HEIGHT, WIDTH } from "./sprite.ts";

const now = Date.parse("2026-09-26T15:00:00Z");
const quiet: Signals = { runs: [], plugins: NO_SIGNALS, update: null, lastInput: now };
const at = (s: Partial<Signals> & { p?: Partial<PluginSignals> }, hour = 15) =>
  pickMood({ ...quiet, ...s, plugins: { ...NO_SIGNALS, ...s.p } }, now, hour);

test("the most urgent thing wins", () => {
  const run = { label: "test", endedAt: null, code: null };
  const failed = { label: "build", endedAt: now - 1000, code: 1 };
  assert.equal(at({ runs: [run, failed], p: { waiting: "Claude in a" } }).mood, "failed");
  assert.equal(at({ runs: [run], p: { waiting: "Claude in a" } }).caption, "Claude in a is waiting for you");
  assert.equal(at({ runs: [run, run], p: { working: "Claude in a" } }).caption, "test is running (+1 more)");
  assert.equal(at({ p: { working: "Claude in a", reading: true } }).mood, "thinking");
  assert.equal(at({ p: { reading: true, review: 2 } }).mood, "reading");
  assert.equal(at({ p: { review: 1 }, update: "1.0" }).caption, "1 waiting for your review");
  assert.equal(at({ p: { review: 0 }, update: "1.0" }).mood, "update");
});

test("what several plugins send is added up", () => {
  assert.deepEqual(combine([]), NO_SIGNALS);
  const all = combine([{ review: 2, reading: false }, { review: 1, waiting: "Claude in a" }, { reading: true, waiting: "Claude in b" }, { review: null }]);
  assert.deepEqual(all, { waiting: "Claude in a", working: null, reading: true, review: 3 });
});

test("runs cheer for a moment after they end", () => {
  assert.equal(at({ runs: [{ label: "t", endedAt: now - 1000, code: 0 }] }).mood, "done");
  assert.equal(at({ runs: [{ label: "t", endedAt: now - RECENT_MS, code: 1 }] }).mood, "idle");
});

test("asleep when you're away, sleepy late at night", () => {
  assert.equal(at({ lastInput: now - SLEEP_MS }).mood, "sleeping");
  assert.equal(at({}, 23).mood, "night");
  assert.equal(at({}, 4).mood, "night");
  assert.equal(at({}, 9).mood, "idle");
  assert.equal(at({ p: { review: null } }).mood, "idle", "nothing said: quiet");
});

test("every character's every frame is a full grid, in colours it has", () => {
  for (const [id, c] of Object.entries(CHARACTERS)) {
    for (const mood of ["failed", "done", "waiting", "running", "thinking", "reading", "review", "update", "sleeping", "night", "idle"] as const) {
      for (let tick = 0; tick < 30; tick++) {
        const rows = frame(mood, tick, c);
        assert.equal(rows.length, HEIGHT, `${id} ${mood}`);
        for (const r of rows) {
          assert.equal(r.length, WIDTH, `${id} ${mood} ${tick}: ${r}`);
          for (const ch of r) assert.ok(ch === "." || c.colors[ch], `${id} ${mood}: no colour for ${ch}`);
        }
      }
    }
  }
});

test("waving changes the picture", () => {
  for (const [id, c] of Object.entries(CHARACTERS)) {
    assert.notDeepEqual(frame("waiting", 0, c), frame("waiting", 2, c), id);
  }
});

test("the face follows the mood", () => {
  assert.equal(frame("idle", 1)[4].slice(4, 6), "wk", "eyes open");
  assert.equal(frame("sleeping", 1)[4].slice(4, 6), "ff", "eyes closed");
  assert.equal(frame("done", 1)[7].slice(7, 9), "oo", "mouth open");
  assert.equal(frame("idle", 1)[4].slice(10, 12), "kw", "the right eye is mirrored");
});
