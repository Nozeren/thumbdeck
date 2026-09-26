export interface Project {
  name: string;
  path: string;
  branch: string | null;
  dirty: boolean;
  /** Scan folder it was found in; null when added by hand */
  root: string | null;
  /** django, android, tauri, node, rust, go, nvim, python or folder */
  kind: string;
  hidden: boolean;
  pinned: boolean;
}

export interface ProjectList {
  projects: Project[];
  /** Folders scanned for git repositories */
  roots: string[];
  /** Project selected last, reopened on start */
  last: string | null;
}

export interface Action {
  id: string;
  label: string;
  command: string;
  /** "custom" for your own actions, otherwise where it was detected (npm, django, ...) */
  source: string;
  /** Ask before running */
  confirm: boolean;
}

/** A custom action as stored in the settings */
export interface CustomAction {
  id: string;
  name: string;
  command: string;
  confirm: boolean;
}

export interface Details {
  actions: Action[];
  /** Detected actions you hid */
  hidden: Action[];
  readme: string | null;
}

export interface Run {
  id: number;
  projectPath: string;
  label: string;
  source: string;
  command: string;
  startedAt: number;
  endedAt: number | null;
  /** null while running */
  code: number | null;
  lines: { text: string; stderr: boolean }[];
}
