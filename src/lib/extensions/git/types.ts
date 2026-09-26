// The Git tab: the shapes the Rust side sends (src-tauri/src/extensions/git)

export interface Setup {
  title: string;
  /** How many recent commits to list */
  commits: number;
}

export interface Branch {
  /** null: not on a branch (a detached HEAD) */
  name: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  /** The upstream branch was deleted */
  gone: boolean;
}

export interface Change {
  path: string;
  /** Where a renamed or copied file came from */
  from: string | null;
  /** M modified, A added, D deleted, R renamed, C copied, U conflict, ? untracked, " " none */
  staged: string;
  unstaged: string;
}

export interface Status {
  branch: Branch;
  changes: Change[];
  /** Last fetch, in seconds since 1970 */
  fetched: number | null;
}

export interface Commit {
  hash: string;
  short: string;
  author: string;
  date: string;
  subject: string;
  refs: string;
}

export interface LocalBranch {
  name: string;
  current: boolean;
  upstream: string | null;
  /** "[ahead 1, behind 2]", "[gone]", or "" */
  track: string;
  date: string;
  subject: string;
}

export interface Stash {
  name: string;
  subject: string;
  date: string;
}

export interface Branches {
  branches: LocalBranch[];
  stashes: Stash[];
}
