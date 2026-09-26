// The avatars as pixel art: 16×13 letters, one per pixel, each character with two frames of
// its body (arms, legs, claws or gills moving) and the spots where its eyes and mouth go. The
// face changes with the mood; the frames alternate faster when there's more going on.

import type { Mood } from "./mood.ts";

export const WIDTH = 16;
export const HEIGHT = 13;

export interface Character {
  name: string;
  /** Two frames of the whole body */
  frames: [string[], string[]];
  /** Row of the eyes' top pixels, and the left and right eye's first column (each eye is 2×2) */
  eyes: [number, number, number];
  /** Row and first column of the mouth (2 pixels) */
  mouth: [number, number];
  /** What each letter is painted with */
  colors: Record<string, string>;
  /** Raise (up) or lower an arm to wave */
  wave: (rows: string[], up: boolean) => void;
}

const set = (row: string, col: number, text: string) => row.slice(0, col) + text + row.slice(col + text.length);

/** An arm by the right side: up to the top of the head, or halfway */
function sideArm(rows: string[], up: boolean, top = 1, bottom = 8) {
  for (let r = up ? top : top + 3; r <= bottom; r++) rows[r] = set(rows[r], 15, "b");
  rows[up ? top - 1 : top + 2] = set(rows[up ? top - 1 : top + 2], 14, "b");
}

// Shared colours: k pupils, w eye shine, m mouth, o open mouth, c cheeks, f the face around them
const FACE = { k: "var(--bg-dim)", w: "var(--fg)", m: "var(--bg-dim)", o: "var(--red)", c: "var(--red)" };

const OCTOPUS_HEAD = [
  ".....bbbbbb.....",
  "...bbbbbbbbbb...",
  "..bbbbbbbbbbbb..",
  "..bbbbbbbbbbbb..",
  ".bbbbbbbbbbbbbb.",
  ".bbbbbbbbbbbbbb.",
  ".bbbbbbbbbbbbbb.",
  ".bbccbbbbbbccbb.",
  "..bbbbbbbbbbbb..",
];

const AXOLOTL_BODY = ["...bbbbbbbbbb..."];

export const CHARACTERS: Record<string, Character> = {
  octopus: {
    name: "Octopus",
    frames: [
      [...OCTOPUS_HEAD, "..bb.bb..bb.bb..", ".bb..bb..bb..bb.", ".b..bb....bb..b.", "..b..........b.."],
      [...OCTOPUS_HEAD, "..bb.bb..bb.bb..", "..bb.bb..bb.bb..", ".bb..b....b..bb.", ".b............b."],
    ],
    eyes: [4, 4, 10],
    mouth: [7, 7],
    colors: { ...FACE, b: "var(--orange)", f: "var(--orange)" },
    wave(rows, up) {
      rows[9] = set(rows[9], 13, "...");
      sideArm(rows, up);
    },
  },
  crab: {
    name: "Crab",
    frames: [
      [
        ".b.b........b.b.",
        ".bbb........bbb.",
        "..b..ff..ff..b..",
        "..b..ff..ff..b..",
        "..bb..b..b..bb..",
        "...bbbbbbbbbb...",
        "..bbbbbbbbbbbb..",
        ".bbbccbbbbccbbb.",
        ".bbbbbbbbbbbbbb.",
        "..bbbbbbbbbbbb..",
        ".b.b.b....b.b.b.",
        "b.b.b......b.b.b",
        "................",
      ],
      [
        ".b.b........b.b.",
        ".bbb........bbb.",
        "..b..ff..ff..b..",
        "..b..ff..ff..b..",
        "..bb..b..b..bb..",
        "...bbbbbbbbbb...",
        "..bbbbbbbbbbbb..",
        ".bbbccbbbbccbbb.",
        ".bbbbbbbbbbbbbb.",
        "..bbbbbbbbbbbb..",
        "..b.b.b..b.b.b..",
        ".b.b.b....b.b.b.",
        "................",
      ],
    ],
    eyes: [2, 5, 9],
    mouth: [7, 7],
    // Eyes on stalks: light around the pupils
    colors: { ...FACE, b: "var(--orange)", f: "var(--fg)", w: "var(--bg0)" },
    wave(rows, up) {
      // The right claw snaps open and shut
      rows[0] = set(rows[0], 12, up ? ".b.b" : "..b.");
      rows[1] = set(rows[1], 12, up ? "bbb." : "bbb.");
    },
  },
  keycap: {
    name: "Keycap",
    frames: [
      [
        "................",
        ".bbbbbbbbbbbbbb.",
        ".bffffffffffffb.",
        ".bffffffffffffb.",
        ".bffffffffffffb.",
        ".bffffffffffffb.",
        ".bffffffffffffb.",
        ".bfccffffffccfb.",
        ".bffffffffffffb.",
        ".bbbbbbbbbbbbbb.",
        "bbbbbbbbbbbbbbbb",
        "...b........b...",
        "..bb........bb..",
      ],
      [
        "................",
        ".bbbbbbbbbbbbbb.",
        ".bffffffffffffb.",
        ".bffffffffffffb.",
        ".bffffffffffffb.",
        ".bffffffffffffb.",
        ".bffffffffffffb.",
        ".bfccffffffccfb.",
        ".bffffffffffffb.",
        ".bbbbbbbbbbbbbb.",
        "bbbbbbbbbbbbbbbb",
        "....b......b....",
        "....bb....bb....",
      ],
    ],
    eyes: [4, 5, 9],
    mouth: [7, 7],
    colors: { ...FACE, b: "var(--grey)", f: "var(--fg)", w: "var(--bg2)" },
    wave(rows, up) {
      sideArm(rows, up, 3, 8);
    },
  },
  ghost: {
    name: "Ghost",
    frames: [
      [...OCTOPUS_HEAD.slice(0, 3), ...Array(8).fill(".bbbbbbbbbbbbbb.").map((r, i) => (i === 4 ? ".bbccbbbbbbccbb." : r)), ".bb.bbb..bbb.bb.", ".b...b....b...b."],
      [...OCTOPUS_HEAD.slice(0, 3), ...Array(8).fill(".bbbbbbbbbbbbbb.").map((r, i) => (i === 4 ? ".bbccbbbbbbccbb." : r)), ".bbb.bb..bb.bbb.", "..b...b..b...b.."],
    ],
    eyes: [4, 4, 10],
    mouth: [7, 7],
    colors: { ...FACE, b: "var(--fg)", f: "var(--fg)", w: "var(--bg2)", c: "var(--grey)" },
    wave(rows, up) {
      sideArm(rows, up);
    },
  },
  axolotl: {
    name: "Axolotl",
    frames: [
      [
        "g..............g",
        ".g..bbbbbbbb..g.",
        "gg.bbbbbbbbbb.gg",
        "..bbbbbbbbbbbb..",
        "ggbbbbbbbbbbbbgg",
        "..bbbbbbbbbbbb..",
        ".gbbbbbbbbbbbbg.",
        "..bccbbbbbbccb..",
        ...AXOLOTL_BODY,
        "...bbbbbbbbbb...",
        "..b.bbbbbbbb.b..",
        ".b..b......b..b.",
        "................",
      ],
      [
        ".g............g.",
        ".g..bbbbbbbb..g.",
        ".g.bbbbbbbbbb.g.",
        "..bbbbbbbbbbbb..",
        ".gbbbbbbbbbbbbg.",
        "..bbbbbbbbbbbb..",
        ".gbbbbbbbbbbbbg.",
        "..bccbbbbbbccb..",
        ...AXOLOTL_BODY,
        "...bbbbbbbbbb...",
        "..bbbbbbbbbbbb..",
        "..b.b......b.b..",
        "................",
      ],
    ],
    eyes: [4, 4, 10],
    mouth: [7, 7],
    colors: { ...FACE, b: "var(--blue)", f: "var(--blue)", g: "var(--aqua)" },
    wave(rows, up) {
      // The right gills flap high
      rows[0] = set(rows[0], 14, up ? "gg" : ".g");
      rows[1] = set(rows[1], 14, up ? "gg" : "g.");
    },
  },
};

export const DEFAULT_CHARACTER = "octopus";

/** Each eye's 2×2 pixels (left eye; the right one is mirrored). f: the face around them */
const EYES = {
  open: ["wk", "kk"],
  closed: ["ff", "kk"],
  happy: ["kk", "ff"],
  sad: ["fk", "kk"],
  up: ["kw", "kf"],
} as const;

const MOUTHS = { smile: "mm", open: "oo", none: "ff" } as const;

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

/** How fast each mood moves the body: ticks per frame (0: still) */
const SPEED: Record<Mood, number> = {
  failed: 0, done: 1, waiting: 2, running: 1, thinking: 4, reading: 6, review: 3, update: 3, sleeping: 0, night: 8, idle: 5,
};

/** The rows to draw for a character in a mood at a tick of the animation */
export function frame(mood: Mood, tick: number, character: Character = CHARACTERS[DEFAULT_CHARACTER]): string[] {
  const speed = SPEED[mood];
  const rows = [...character.frames[speed ? Math.floor(tick / speed) % 2 : 0]];
  const f = face(mood, tick);
  const [top, bottom] = EYES[f.eyes];
  const mirror = (s: string) => [...s].reverse().join("");
  const [er, left, right] = character.eyes;
  rows[er] = set(set(rows[er], left, top), right, mirror(top));
  rows[er + 1] = set(set(rows[er + 1], left, bottom), right, mirror(bottom));
  rows[character.mouth[0]] = set(rows[character.mouth[0]], character.mouth[1], MOUTHS[f.mouth]);
  if (mood === "waiting") character.wave(rows, tick % 4 < 2);
  return rows;
}
