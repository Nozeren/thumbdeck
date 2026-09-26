// What a tab asks the review page to show

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
}
