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
  /** "custom" for your own actions, otherwise the toolkit pack's id (npm, django, ...) */
  source: string;
  /** Group title in the Toolkit: the pack's name, or "yours" */
  group: string;
  description: string | null;
  /** Ask before running */
  confirm: boolean;
  /** Runs in this window of the project's tmux session instead of in thumbdeck */
  tmux: string | null;
}

/** A custom action as stored in the settings */
export interface CustomAction {
  id: string;
  name: string;
  command: string;
  confirm: boolean;
  /** Window of the project's tmux session to run in; null runs it inside thumbdeck */
  tmux: string | null;
}

export interface Details {
  actions: Action[];
  /** Detected actions you hid */
  hidden: Action[];
  /** Toolkit packs that couldn't be used, and why */
  problems: string[];
  readme: string | null;
  /** Extension tabs turned on for the project */
  tabs: TabInfo[];
}

/** An extension that can be added to a project as a tab */
export interface Extension {
  id: string;
  name: string;
  description: string;
}

/** An extension turned on for a project, with its setup */
export interface Tab {
  extension: string;
  setup: any; // the extension's own (e.g. the Logs tab's Setup)
}

export interface TabInfo extends Tab {
  title: string;
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

/** A newer release of thumbdeck */
export interface Update {
  version: string;
  current: string;
  /** What's new (markdown) */
  notes: string;
  date: string;
}
