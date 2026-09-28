// What the avatar in the top bar shows: one mood, picked from what's going on, most urgent
// first. Pure, tested with `node --test` (mood.test.ts).

export type Mood =
  | "failed" | "done" | "waiting" | "running" | "thinking" | "reading" | "review" | "update" | "sleeping" | "night" | "idle";

/** What plugins tell the avatar (td.ui.mood), added up over every frame and backend */
export interface PluginSignals {
  /** Who waits for you ("Claude in shop") */
  waiting: string | null;
  /** Who's working */
  working: string | null;
  /** Something is being read (a log) */
  reading: boolean;
  /** Things waiting for your review (null: nobody said) */
  review: number | null;
}

export const NO_SIGNALS: PluginSignals = { waiting: null, working: null, reading: false, review: null };

/** What each frame or backend sent (signal -> value), as one set */
export function combine(sent: Record<string, unknown>[]): PluginSignals {
  const out = { ...NO_SIGNALS };
  for (const s of sent) {
    if (typeof s.waiting === "string" && s.waiting && !out.waiting) out.waiting = s.waiting;
    if (typeof s.working === "string" && s.working && !out.working) out.working = s.working;
    if (s.reading === true) out.reading = true;
    if (typeof s.review === "number") out.review = (out.review ?? 0) + s.review;
  }
  return out;
}

export interface Signals {
  runs: { label: string; endedAt: number | null; code: number | null }[];
  plugins: PluginSignals;
  /** A newer thumbdeck, if there is one */
  update: string | null;
  /** Your last key or click */
  lastInput: number;
}

/** A run that ended this recently still gets its cheer (or its sigh) */
export const RECENT_MS = 6_000;
/** No keys or clicks for this long: asleep */
export const SLEEP_MS = 10 * 60_000;

export function pickMood(s: Signals, now: number, hour = new Date(now).getHours()): { mood: Mood; caption: string } {
  const recent = s.runs.filter((r) => r.endedAt !== null && now - r.endedAt < RECENT_MS).sort((a, b) => b.endedAt! - a.endedAt!)[0];
  if (recent && recent.code !== 0) return { mood: "failed", caption: `${recent.label} failed` };
  if (recent) return { mood: "done", caption: `${recent.label} finished` };

  if (s.plugins.waiting) return { mood: "waiting", caption: `${s.plugins.waiting} is waiting for you` };

  const running = s.runs.filter((r) => r.endedAt === null);
  if (running.length) {
    const more = running.length > 1 ? ` (+${running.length - 1} more)` : "";
    return { mood: "running", caption: `${running[0].label} is running${more}` };
  }

  if (s.plugins.working) return { mood: "thinking", caption: `${s.plugins.working} is working` };
  if (s.plugins.reading) return { mood: "reading", caption: "Reading" };
  if (s.plugins.review) return { mood: "review", caption: `${s.plugins.review} waiting for your review` };
  if (s.update) return { mood: "update", caption: `thumbdeck ${s.update} is available` };
  if (now - s.lastInput >= SLEEP_MS) return { mood: "sleeping", caption: "Asleep: nothing happened for a while" };
  if (hour >= 23 || hour < 5) return { mood: "night", caption: "It's late…" };
  return { mood: "idle", caption: "All quiet" };
}
