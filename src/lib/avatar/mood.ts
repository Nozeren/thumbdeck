// What the avatar in the top bar shows: one mood, picked from what's going on, most urgent
// first. Pure, tested with `node --test` (mood.test.ts).

export type Mood =
  | "failed" | "done" | "waiting" | "running" | "thinking" | "reading" | "review" | "update" | "sleeping" | "night" | "idle";

export interface Signals {
  runs: { label: string; endedAt: number | null; code: number | null }[];
  /** Claude Code sessions running now: their project's name and status (busy, idle, waiting) */
  claude: { project: string; status: string }[];
  /** The Logs tab is reading a log */
  logsReading: boolean;
  /** PRs asking for your review (null: no Pull requests tab has looked) */
  prsToReview: number | null;
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

  const waiting = s.claude.find((c) => c.status === "waiting");
  if (waiting) return { mood: "waiting", caption: `Claude is waiting for you in ${waiting.project}` };

  const running = s.runs.filter((r) => r.endedAt === null);
  if (running.length) {
    const more = running.length > 1 ? ` (+${running.length - 1} more)` : "";
    return { mood: "running", caption: `${running[0].label} is running${more}` };
  }

  const busy = s.claude.find((c) => c.status === "busy");
  if (busy) return { mood: "thinking", caption: `Claude is working in ${busy.project}` };
  if (s.logsReading) return { mood: "reading", caption: "Reading a log" };
  if (s.prsToReview) return { mood: "review", caption: `${s.prsToReview} PR${s.prsToReview === 1 ? "" : "s"} to review` };
  if (s.update) return { mood: "update", caption: `thumbdeck ${s.update} is available` };
  if (now - s.lastInput >= SLEEP_MS) return { mood: "sleeping", caption: "Asleep: nothing happened for a while" };
  if (hour >= 23 || hour < 5) return { mood: "night", caption: "It's late…" };
  return { mood: "idle", caption: "All quiet" };
}
