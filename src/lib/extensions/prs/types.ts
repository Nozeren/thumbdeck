// The Pull requests tab: the shapes the Rust side sends (src-tauri/src/extensions/prs)

export interface Setup {
  title: string;
  /** "owner/name"; empty: from the project's git remote; "demo": sample PRs */
  repo: string;
  stale_days: number;
  /** Reviewers left out (bots) */
  bots: string[];
  /** Cut from the end of user names */
  trim_suffix: string;
  refresh_minutes: number;
}

export type Category = "re-review" | "review" | "reviewed" | "mine";

export interface Pr {
  category: Category;
  number: number;
  title: string;
  url: string;
  author: string;
  updated: string;
  draft: boolean;
  /** SUCCESS, FAILURE, PENDING, ... or "" (no checks) */
  checks: string;
  /** MERGEABLE, CONFLICTING or UNKNOWN */
  mergeable: string;
  labels: string[];
  comments: number;
  last_comment: { author: string; updated: string } | null;
  /** APPROVED, CHANGES_REQUESTED, COMMENTED, ... or PENDING */
  reviewers: { name: string; state: string }[];
  /** An unread GitHub notification about it */
  notification: string | null;
}

export interface PrList {
  repo: string;
  /** Sample PRs, not from GitHub (repo "demo") */
  demo: boolean;
  user: string;
  prs: Pr[];
}
