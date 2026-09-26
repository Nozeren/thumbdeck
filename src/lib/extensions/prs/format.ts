// Small pure helpers for the Pull requests tab, tested with `node --test` (format.test.ts).

/** Not updated for `days` days or more */
export function isStale(updated: string, days: number, now = Date.now()): boolean {
  const t = Date.parse(updated);
  return !Number.isNaN(t) && days > 0 && now - t >= days * 86400_000;
}

export const categoryLabel: Record<string, string> = { "re-review": "RE-REVIEW", review: "REVIEW", reviewed: "REVIEWED", mine: "MINE" };

/** A check or review state in words, with its sign */
export function checksText(state: string): string {
  return ({ SUCCESS: "✔ passed", FAILURE: "✘ failed", ERROR: "✘ error", PENDING: "⏳ pending", EXPECTED: "⏳ expected" } as Record<string, string>)[state] ?? "no checks";
}

export function reviewText(state: string): string {
  return ({ APPROVED: "✔ approved", CHANGES_REQUESTED: "✘ changes requested", COMMENTED: "💬 commented", DISMISSED: "dismissed", PENDING: "⏳ pending" } as Record<string, string>)[state] ?? state.toLowerCase();
}

/** "a, b ,,c" -> ["a", "b", "c"] */
export const splitList = (text: string) => text.split(",").map((s) => s.trim()).filter(Boolean);
