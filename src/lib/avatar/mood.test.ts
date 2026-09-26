import { test } from "node:test";
import assert from "node:assert/strict";
import { pickMood, RECENT_MS, SLEEP_MS, type Signals } from "./mood.ts";
import { frame, WIDTH } from "./sprite.ts";

const now = Date.parse("2026-09-26T15:00:00Z");
const quiet: Signals = { runs: [], claude: [], logsReading: false, prsToReview: null, update: null, lastInput: now };
const at = (s: Partial<Signals>, hour = 15) => pickMood({ ...quiet, ...s }, now, hour);

test("the most urgent thing wins", () => {
  const run = { label: "test", endedAt: null, code: null };
  const failed = { label: "build", endedAt: now - 1000, code: 1 };
  assert.equal(at({ runs: [run, failed], claude: [{ project: "a", status: "waiting" }] }).mood, "failed");
  assert.equal(at({ runs: [run], claude: [{ project: "a", status: "waiting" }] }).caption, "Claude is waiting for you in a");
  assert.equal(at({ runs: [run, run], claude: [{ project: "a", status: "busy" }] }).caption, "test is running (+1 more)");
  assert.equal(at({ claude: [{ project: "a", status: "busy" }], logsReading: true }).mood, "thinking");
  assert.equal(at({ logsReading: true, prsToReview: 2 }).mood, "reading");
  assert.equal(at({ prsToReview: 1, update: "1.0" }).caption, "1 PR to review");
  assert.equal(at({ prsToReview: 0, update: "1.0" }).mood, "update");
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
  assert.equal(at({ claude: [{ project: "a", status: "idle" }] }).mood, "idle", "an idle session is quiet");
});

test("every frame is a full grid", () => {
  for (const mood of ["failed", "done", "waiting", "running", "thinking", "reading", "review", "update", "sleeping", "night", "idle"] as const) {
    for (let tick = 0; tick < 30; tick++) {
      const rows = frame(mood, tick);
      assert.equal(rows.length, 13, mood);
      for (const r of rows) assert.equal(r.length, WIDTH, `${mood} ${tick}: ${r}`);
    }
  }
});

test("the face follows the mood", () => {
  assert.equal(frame("idle", 1)[4].slice(4, 6), "wk", "eyes open");
  assert.equal(frame("sleeping", 1)[4].slice(4, 6), "bb", "eyes closed");
  assert.equal(frame("done", 1)[7].slice(7, 9), "oo", "mouth open");
  assert.equal(frame("idle", 1)[4].slice(10, 12), "kw", "the right eye is mirrored");
});
