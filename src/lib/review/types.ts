// What a tab asks the review page to show (and, for a pull request, what it can do there)

export interface ReviewRequest {
  /** "Uncommitted changes", "a1b2c3d Fix login", "#412 Fix login redirect" */
  title: string;
  /** A second line: author, branch, when */
  subtitle?: string;
  /** Gets the unified diff (again, on r) */
  load: () => Promise<string>;
  /** Where the files you marked viewed are remembered ("pr:owner/name#412") */
  viewedKey: string;
  /** Open at this file */
  startFile?: string;
  /** A file's whole text in the new version: the unchanged parts can then be opened (f) */
  file?: (path: string) => Promise<string>;
  /** About the pull request, for its pane (i) and the header */
  about?: ReviewAbout;
  /** What can be done on GitHub; without it the page is read-only */
  actions?: ReviewActions;
}

export interface ReviewAbout {
  /** Its description (Markdown) */
  description?: string;
  author?: string;
  head?: string;
  base?: string;
  /** Its page, for o */
  url?: string;
  checks?: { name: string; state: "success" | "failure" | "pending" | "neutral"; detail?: string }[];
  /** state: "approved", "changes requested", "commented", "asked" */
  reviewers?: { name: string; state: string }[];
  labels?: string[];
}

/** A discussion on lines of the diff */
export interface ReviewThread {
  id: string;
  path: string;
  /** Where it sits (null: its lines changed since, an outdated thread) */
  line: number | null;
  startLine?: number | null;
  side: "LEFT" | "RIGHT";
  resolved: boolean;
  comments: { author: string; body: string; createdAt?: string }[];
}

/** A comment of the review being written, as GitHub takes it */
export interface ReviewComment {
  path: string;
  line: number;
  side: "LEFT" | "RIGHT";
  startLine?: number;
  startSide?: "LEFT" | "RIGHT";
  body: string;
}

export type ReviewEvent = "COMMENT" | "APPROVE" | "REQUEST_CHANGES";

export interface ReviewActions {
  /** The review threads (read again after each change) */
  threads?: () => Promise<ReviewThread[]>;
  /** Send the review: a verdict, a summary and the line comments */
  submit?: (review: { event: ReviewEvent; body: string; comments: ReviewComment[] }) => Promise<void>;
  /** Answer a thread (sent at once, not with the review) */
  reply?: (threadId: string, body: string) => Promise<void>;
  /** Mark a thread resolved, or not */
  resolve?: (threadId: string, resolved: boolean) => Promise<void>;
}
