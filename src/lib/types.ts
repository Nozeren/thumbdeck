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
  /** Folded sections of the tree: "pinned", "added", "hidden" or a scan folder */
  collapsed: string[];
}

export interface Action {
  id: string;
  label: string;
  command: string;
  source: string;
}

export interface Details {
  actions: Action[];
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
