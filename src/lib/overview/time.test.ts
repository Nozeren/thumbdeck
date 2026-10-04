import { test } from "node:test";
import assert from "node:assert/strict";
import { ago, duration } from "./time.ts";

const now = Date.UTC(2026, 9, 4, 12, 0, 0);

test("how long ago", () => {
  assert.equal(ago(now - 20_000, now), "just now");
  assert.equal(ago(now - 5 * 60_000, now), "5m ago");
  assert.equal(ago(now - 3 * 3600_000, now), "3h ago");
  assert.equal(ago(now - 30 * 3600_000, now), "yesterday");
  assert.equal(ago(now - 4 * 86400_000, now), "4d ago");
  assert.equal(ago(now + 5000, now), "just now", "a clock a little ahead");
});

test("durations", () => {
  assert.equal(duration(12), "12s");
  assert.equal(duration(184), "3m 4s");
  assert.equal(duration(3725), "1h 2m");
});
