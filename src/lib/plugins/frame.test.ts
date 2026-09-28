import { test } from "node:test";
import assert from "node:assert/strict";
import { base64url, frameUrl } from "./frame.ts";
import type { FrameInfo } from "./types.ts";

const info = { plugin: "git", version: "1.0.0", folder: "/p/git", data_folder: "/d/git", page: "ui/tab page.html" } as FrameInfo;

test("the context travels in the address, readable back as UTF-8 JSON", () => {
  const url = frameUrl(info, { surface: "tab", id: "git", project: { path: "/home/me/dev/café", name: "café", branch: null }, api: 1, thumbdeck: "0.5.0" });
  assert.ok(url.startsWith("plugin://git/ui/tab%20page.html?td="));
  const raw = url.split("td=")[1];
  assert.ok(!/[+/=]/.test(raw), "base64url, no padding");
  const bytes = Uint8Array.from(atob(raw.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));
  const ctx = JSON.parse(new TextDecoder().decode(bytes));
  assert.equal(ctx.project.name, "café");
  assert.deepEqual(ctx.plugin, { id: "git", version: "1.0.0", folder: "/p/git", dataFolder: "/d/git" });
  assert.equal("data" in ctx, false);
});

test("base64url", () => {
  assert.equal(base64url("??>"), "Pz8-");
  assert.equal(base64url("a"), "YQ");
});
