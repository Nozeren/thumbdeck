export interface Project {
  name: string;
  path: string;
  branch: string | null;
  dirty: boolean;
  /** Added by hand rather than found in a scan folder */
  added: boolean;
  hidden: boolean;
}

export interface ProjectList {
  projects: Project[];
  /** Folders scanned for git repositories */
  roots: string[];
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
