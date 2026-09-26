// The octopus as pixel art: 16×13 rows of letters, one per pixel (see COLORS in
// Octopus.svelte). Its face and arms change with the mood and the animation's tick.

import type { Mood } from "./mood.ts";

export const WIDTH = 16;

const HEAD = [
  ".....bbbbbb.....",
  "...bbbbbbbbbb...",
  "..bbbbbbbbbbbb..",
  "..bbbbbbbbbbbb..",
  ".bbbEEbbbbEEbbb.", // eyes: columns 4-5 and 10-11, rows 4-5
  ".bbbEEbbbbEEbbb.",
  ".bbbbbbbbbbbbbb.",
  ".bbccbbMMbbccbb.", // mouth: columns 7-8
  "..bbbbbbbbbbbb..",
];

const ARMS = [
  ["..bb.bb..bb.bb..", ".bb..bb..bb..bb.", ".b..bb....bb..b.", "..b..........b.."],
  ["..bb.bb..bb.bb..", "..bb.bb..bb.bb..", ".bb..b....b..bb.", ".b............b."],
];

/** Each eye's 2×2 pixels (left eye; the right one is mirrored) */
const EYES = {
  open: ["wk", "kk"],
  closed: ["bb", "kk"],
  happy: ["kk", "bb"],
  sad: ["bk", "kk"],
  up: ["kw", "kb"],
} as const;

const MOUTHS = { smile: "mm", open: "oo", none: "bb" } as const;

type Face = { eyes: keyof typeof EYES; mouth: keyof typeof MOUTHS };

function face(mood: Mood, tick: number): Face {
  const blink = tick % 24 === 0;
  switch (mood) {
    case "failed": return { eyes: "sad", mouth: "open" };
    case "done": return { eyes: "happy", mouth: "open" };
    case "waiting": return { eyes: "open", mouth: tick % 4 < 2 ? "open" : "smile" };
    case "running": return { eyes: blink ? "closed" : "open", mouth: "smile" };
    case "thinking": return { eyes: "up", mouth: "none" };
    case "reading": return { eyes: tick % 16 < 8 ? "open" : "up", mouth: "none" };
    case "sleeping": return { eyes: "closed", mouth: "none" };
    case "night": return { eyes: "closed", mouth: tick % 20 < 5 ? "open" : "none" };
    default: return { eyes: blink ? "closed" : "open", mouth: "smile" };
  }
}

/** How fast each mood's arms move: ticks per arm frame (0: still) */
const ARM_SPEED: Record<Mood, number> = {
  failed: 0, done: 1, waiting: 2, running: 1, thinking: 4, reading: 6, review: 3, update: 3, sleeping: 0, night: 8, idle: 5,
};

const set = (row: string, col: number, text: string) => row.slice(0, col) + text + row.slice(col + text.length);

/** The rows to draw for a mood at a tick of the animation */
export function frame(mood: Mood, tick: number): string[] {
  const f = face(mood, tick);
  const [top, bottom] = EYES[f.eyes];
  const mirror = (s: string) => [...s].reverse().join("");
  const rows = HEAD.map((r) => r);
  rows[4] = set(set(rows[4], 4, top), 10, mirror(top));
  rows[5] = set(set(rows[5], 4, bottom), 10, mirror(bottom));
  rows[7] = set(rows[7], 7, MOUTHS[f.mouth]);
  const speed = ARM_SPEED[mood];
  const arms = ARMS[speed ? Math.floor(tick / speed) % 2 : 0].map((r) => r);
  if (mood === "waiting") {
    // One arm waving next to the head: up, then halfway down
    arms[0] = set(arms[0], 13, "...");
    const up = tick % 4 < 2;
    for (let r = up ? 1 : 4; r <= 8; r++) rows[r] = set(rows[r], 15, "b");
    rows[up ? 0 : 3] = set(rows[up ? 0 : 3], 14, "b");
  }
  return [...rows, ...arms];
}
